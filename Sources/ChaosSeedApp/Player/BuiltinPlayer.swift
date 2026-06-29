import SwiftUI
import AVFoundation
import AVKit
import AppKit
import CPrivatePIP

/// 播放器选择偏好。
public enum PlayerPreference: String, CaseIterable, Codable {
    case builtin
    case iina

    public var label: String {
        switch self {
        case .builtin: return "内置播放器"
        case .iina: return "IINA"
        }
    }
}

/// 一次内置播放会话首次真正开始播放时发布的确认事件。
///
/// 同一会话切换备用线路不会生成新事件，避免历史记录重复写入。
public struct PlaybackConfirmation: Equatable, Sendable {
    public let sessionID: UUID

    public init(sessionID: UUID) {
        self.sessionID = sessionID
    }
}

/// 内置播放器：HLS / MP4 使用 AVPlayer，HTTP-FLV 优先使用内嵌 libmpv。
///
/// - HTTP header 注入：通过 `AVURLAssetHTTPHeaderFieldsKey` 设置 Referer / UA，解决 Bili CDN 校验。
/// - HDR / Dolby：由 AVFoundation、媒体编码、显示器与音频输出设备共同决定；
///   应用不伪造能力标识，也不把时间拉伸算法当作 Atmos 开关。
///
/// 用法：
/// ```swift
/// let player = BuiltinPlayer()
/// player.play(url: streamURL, hints: PlaybackHints(referer: "https://live.bilibili.com/", userAgent: biliUA))
/// // 用 BuiltinPlayerView(player: player) 呈现视频画面
/// ```
@MainActor
public final class BuiltinPlayer: ObservableObject, BuiltinPlayable {
    /// 底层播放器在视图生命周期内保持同一实例，避免渲染层更新期间反复解绑。
    public let avPlayer = AVPlayer()
    @Published public var isPlaying = false
    @Published public private(set) var volume: Float = 1
    @Published public private(set) var isMuted = false
    @Published public var errorMessage: String?
    @Published public private(set) var engine: BuiltinPlaybackEngine = .avFoundation
    @Published public private(set) var webFLVRequest: WebFLVPlaybackRequest?
    @Published private(set) var libMPVPlayer: LibMPVPlayerCore?
    @Published public private(set) var activeSource: BuiltinPlaybackSource?
    @Published public private(set) var activeSourceIndex = 0
    @Published public private(set) var sourceCount = 0
    @Published public private(set) var isPictureInPicturePossible = false
    @Published public private(set) var isPictureInPictureActive = false
    @Published public private(set) var pictureInPictureError: String?
    @Published public private(set) var playbackConfirmation: PlaybackConfirmation?

    private var itemStatusObservation: NSKeyValueObservation?
    private var pictureInPicturePossibleObservation: NSKeyValueObservation?
    private var pictureInPictureLayerReadyObservation: NSKeyValueObservation?
    private var playbackSources: [BuiltinPlaybackSource] = []
    private var playbackHints = PlaybackHints()
    private var attemptErrors: [String] = []
    private var attemptID = UUID()
    private var openTimeoutTask: Task<Void, Never>?
    private var pictureInPictureAvailabilityTask: Task<Void, Never>?
    private var pictureInPictureStartTask: Task<Void, Never>?
    private var pictureInPictureController: AVPictureInPictureController?
    private var privatePIPSession: UnsafeMutableRawPointer?
    private var appPIPSession: AppOwnedLibMPVPIPSession?
    private weak var pictureInPicturePlayerLayer: AVPlayerLayer?
    private weak var libMPVView: LibMPVOpenGLView?
    private lazy var pictureInPictureDelegate = BuiltinPictureInPictureDelegate(player: self)
    private var nativePictureInPicturePossible = false
    private var webPictureInPicturePossible = false
    private var webPictureInPictureRegistrationID: UUID?
    private var webPlaybackActions: WebPlaybackActions?
    private var playbackSessionID = UUID()
    private var confirmedPlaybackSessionID: UUID?

    public var playerView: AnyView? {
        AnyView(BuiltinPlayerView(player: self))
    }

    /// 将当前稳定的 `AVPlayerLayer` 注册为 macOS 系统画中画内容源。
    ///
    /// 控制器由播放器强持有，避免 SwiftUI 重绘导致画中画会话提前释放。
    func attachPictureInPicture(to playerLayer: AVPlayerLayer) {
        guard AVPictureInPictureController.isPictureInPictureSupported() else {
            nativePictureInPicturePossible = false
            refreshPictureInPictureAvailability()
            return
        }
        if pictureInPicturePlayerLayer === playerLayer {
            return
        }

        pictureInPicturePlayerLayer = playerLayer
        pictureInPictureLayerReadyObservation?.invalidate()
        pictureInPicturePossibleObservation?.invalidate()
        pictureInPictureController?.stopPictureInPicture()
        pictureInPictureController = nil
        nativePictureInPicturePossible = false
        refreshPictureInPictureAvailability()

        pictureInPictureLayerReadyObservation = playerLayer.observe(
            \.isReadyForDisplay,
            options: [.initial, .new]
        ) { [weak self] layer, _ in
            guard layer.isReadyForDisplay else { return }
            Task { @MainActor [weak self] in
                guard let self,
                      let playerLayer = self.pictureInPicturePlayerLayer,
                      self.pictureInPictureController == nil else {
                    return
                }
                self.installPictureInPictureController(for: playerLayer)
            }
        }
    }

    /// 只有视频图层真正可显示后才创建控制器；过早创建会让
    /// `isPictureInPicturePossible` 长期停留在 false。
    private func installPictureInPictureController(for playerLayer: AVPlayerLayer) {
        guard let controller = AVPictureInPictureController(playerLayer: playerLayer) else {
            nativePictureInPicturePossible = false
            pictureInPictureError = "系统无法为当前视频创建画中画控制器"
            refreshPictureInPictureAvailability()
            return
        }
        controller.delegate = pictureInPictureDelegate
        pictureInPictureController = controller
        // macOS 27 Beta 对自定义 AVPlayerLayer 的 possible 状态存在持续误报。
        // 图层已经 ready 时允许用户发起，由系统 delegate 返回真实结果。
        nativePictureInPicturePossible = playerLayer.isReadyForDisplay
        pictureInPicturePossibleObservation = controller.observe(
            \.isPictureInPicturePossible,
            options: [.initial, .new]
        ) { [weak self] controller, _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.nativePictureInPicturePossible = controller.isPictureInPicturePossible
                    || self.pictureInPicturePlayerLayer?.isReadyForDisplay == true
                self.refreshPictureInPictureAvailability()
            }
        }
        refreshPictureInPictureAvailability()
        schedulePictureInPictureAvailabilityChecks(for: controller)
    }

    /// 注册 WebKit HTTP-FLV 的播放控制动作。
    func attachWebPlaybackActions(id: UUID, actions: WebPlaybackActions) {
        webPictureInPictureRegistrationID = id
        webPlaybackActions = actions
        refreshPictureInPictureAvailability()
    }

    func detachWebPlaybackActions(id: UUID) {
        guard webPictureInPictureRegistrationID == id else { return }
        webPictureInPictureRegistrationID = nil
        webPlaybackActions = nil
        webPictureInPicturePossible = false
        refreshPictureInPictureAvailability()
    }

    func webPictureInPictureAvailabilityChanged(_ isPossible: Bool, id: UUID) {
        guard webPictureInPictureRegistrationID == id else { return }
        webPictureInPicturePossible = isPossible
        refreshPictureInPictureAvailability()
    }

    func webPictureInPictureStateChanged(_ isActive: Bool, id: UUID) {
        guard engine == .webFLV, webPictureInPictureRegistrationID == id else { return }
        if isPictureInPictureActive != isActive {
            isPictureInPictureActive = isActive
        }
        pictureInPictureError = nil
    }

    func webPictureInPictureDidFail(_ message: String, id: UUID) {
        guard engine == .webFLV, webPictureInPictureRegistrationID == id else { return }
        pictureInPictureError = message
        isPictureInPictureActive = false
    }

    /// 切换当前播放后端的画中画状态。
    public func togglePictureInPicture() {
        pictureInPictureError = nil
        switch engine {
        case .avFoundation:
            guard let controller = pictureInPictureController else {
                pictureInPictureError = "视频渲染层尚未准备好"
                return
            }
            if controller.isPictureInPictureActive {
                controller.stopPictureInPicture()
            } else {
                controller.startPictureInPicture()
                schedulePictureInPictureStartCheck(for: controller)
            }
        case .libMPV:
            togglePrivateLibMPVPictureInPicture()
        case .webFLV:
            guard webPictureInPicturePossible, let action = webPlaybackActions?.togglePictureInPicture else {
                pictureInPictureError = "当前 HTTP-FLV 视频暂时无法进入画中画"
                return
            }
            action()
        }
    }

    /// 按顺序尝试同一清晰度的全部主链/备链，单条打开失败或超时后自动切换。
    public func play(sources: [BuiltinPlaybackSource], hints: PlaybackHints) {
        let unique = sources.reduce(into: [BuiltinPlaybackSource]()) { result, source in
            guard !result.contains(source) else { return }
            result.append(source)
        }
        guard !unique.isEmpty else {
            stopPlaybackAttempt()
            errorMessage = "当前清晰度没有内置播放器支持的地址"
            return
        }

        stopPlaybackAttempt()
        playbackSources = unique
        playbackHints = hints
        playbackSessionID = UUID()
        confirmedPlaybackSessionID = nil
        playbackConfirmation = nil
        sourceCount = unique.count
        activeSourceIndex = 0
        attemptErrors = []
        errorMessage = nil
        playCurrentSource()
    }

    /// 启动指定后端的内置播放。
    public func play(source: BuiltinPlaybackSource, hints: PlaybackHints) {
        play(sources: [source], hints: hints)
    }

    /// `LivePlayer` 兼容入口，明确按 AVFoundation 媒体处理。
    public func play(url: URL, hints: PlaybackHints) {
        play(source: BuiltinPlaybackSource(url: url, engine: .avFoundation), hints: hints)
    }

    private func playCurrentSource() {
        guard playbackSources.indices.contains(activeSourceIndex) else {
            finishWithPlaybackError()
            return
        }
        let source = playbackSources[activeSourceIndex]
        let currentAttemptID = UUID()
        attemptID = currentAttemptID
        activeSource = source
        isPlaying = false
        errorMessage = nil

        switch source.engine {
        case .avFoundation:
            playAVFoundation(url: source.url, hints: playbackHints, attemptID: currentAttemptID)
        case .libMPV:
            playLibMPV(url: source.url, hints: playbackHints)
        case .webFLV:
            playWebFLV(url: source.url, hints: playbackHints)
        }
        scheduleOpenTimeout(for: currentAttemptID)
    }

    /// 启动 AVFoundation 播放：设置 HTTP header 并开始拉流。
    ///
    /// 系统媒体能力：
    /// - E-AC-3 / Atmos 是否输出取决于流本身、系统解码器与当前音频设备；
    /// - HDR / Dolby Vision 是否呈现取决于视频元数据、显示器和 AVPlayer 渲染链。
    private func playAVFoundation(
        url: URL,
        hints: PlaybackHints,
        attemptID currentAttemptID: UUID
    ) {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()
        libMPVPlayer?.stop()
        engine = .avFoundation
        webFLVRequest = nil
        webPictureInPicturePossible = false
        webPictureInPictureRegistrationID = nil
        webPlaybackActions = nil
        refreshPictureInPictureAvailability()

        // 构造 HTTP 请求 header（Bili CDN 需校验 Referer + UA）。
        var headers: [String: String] = [:]
        if let referer = hints.referer {
            headers["Referer"] = referer
        }
        if let ua = hints.userAgent {
            headers["User-Agent"] = ua
        }

        // 通过 AVURLAssetHTTPHeaderFieldsKey 注入 header。
        let options = headers.isEmpty ? nil : ["AVURLAssetHTTPHeaderFieldsKey": headers]
        let asset = AVURLAsset(url: url, options: options)

        let playerItem = AVPlayerItem(asset: asset)
        playerItem.preferredForwardBufferDuration = 2.0

        // 播放器配置：自动播放、不静音。
        avPlayer.automaticallyWaitsToMinimizeStalling = true
        avPlayer.volume = volume
        avPlayer.isMuted = isMuted
        avPlayer.replaceCurrentItem(with: playerItem)

        self.errorMessage = nil
        self.isPlaying = false

        itemStatusObservation = playerItem.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
            Task { @MainActor in
                guard let self, self.attemptID == currentAttemptID else { return }
                switch item.status {
                case .readyToPlay:
                    self.isPlaying = true
                    self.confirmPlaybackIfNeeded()
                    self.openTimeoutTask?.cancel()
                    self.openTimeoutTask = nil
                    if let controller = self.pictureInPictureController {
                        self.schedulePictureInPictureAvailabilityChecks(for: controller)
                    }
                case .failed:
                    self.advanceToNextSource(
                        after: item.error?.localizedDescription ?? "AVFoundation 无法读取直播流"
                    )
                case .unknown:
                    break
                @unknown default:
                    break
                }
            }
        }

        avPlayer.play()
    }

    /// 启动内嵌 libmpv 播放，主要用于 HTTP-FLV。
    private func playLibMPV(url: URL, hints: PlaybackHints) {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()
        if pictureInPictureController?.isPictureInPictureActive == true {
            pictureInPictureController?.stopPictureInPicture()
        }

        let core: LibMPVPlayerCore
        if let current = libMPVPlayer {
            core = current
        } else {
            core = LibMPVPlayerCore(
                onPlaybackStarted: { [weak self] _ in
                    Task { @MainActor [weak self] in
                        self?.libMPVDidStartPlaying()
                    }
                },
                onPlaybackFailed: { [weak self] message in
                    Task { @MainActor [weak self] in
                        self?.libMPVDidFail(message ?? "libmpv 播放失败")
                    }
                }
            )
            libMPVPlayer = core
        }

        errorMessage = nil
        isPlaying = false
        engine = .libMPV
        webFLVRequest = nil
        webPictureInPicturePossible = false
        webPictureInPictureRegistrationID = nil
        webPlaybackActions = nil
        isPictureInPictureActive = false
        refreshPictureInPictureAvailability()

        if let message = core.play(url: url, hints: hints) {
            advanceToNextSource(after: message)
        }
    }

    /// 启动应用内 HTTP-FLV 播放，由 WebKit 中的 mpegts.js 转封装到 MSE。
    private func playWebFLV(url: URL, hints: PlaybackHints) {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()
        libMPVPlayer?.stop()
        if pictureInPictureController?.isPictureInPictureActive == true {
            pictureInPictureController?.stopPictureInPicture()
        }

        var headers: [String: String] = [:]
        if let referer = hints.referer {
            headers["Referer"] = referer
        }
        if let userAgent = hints.userAgent {
            headers["User-Agent"] = userAgent
        }

        errorMessage = nil
        isPlaying = false
        engine = .webFLV
        webFLVRequest = WebFLVPlaybackRequest(url: url, headers: headers)
        webPictureInPicturePossible = false
        isPictureInPictureActive = false
        refreshPictureInPictureAvailability()
    }

    func webFLVDidStartPlaying() {
        guard engine == .webFLV else { return }
        openTimeoutTask?.cancel()
        openTimeoutTask = nil
        isPlaying = true
        errorMessage = nil
        confirmPlaybackIfNeeded()
    }

    func webFLVDidPause() {
        guard engine == .webFLV else { return }
        isPlaying = false
    }

    func webFLVDidFail(_ message: String) {
        guard engine == .webFLV else { return }
        advanceToNextSource(after: message.isEmpty ? "HTTP-FLV 播放失败" : message)
    }

    func libMPVDidStartPlaying() {
        guard engine == .libMPV else { return }
        openTimeoutTask?.cancel()
        openTimeoutTask = nil
        isPlaying = true
        errorMessage = nil
        if let session = privatePIPSession {
            ChaosPrivatePIPSetPlaying(session, true)
        }
        confirmPlaybackIfNeeded()
    }

    func libMPVDidFail(_ message: String) {
        guard engine == .libMPV else { return }
        advanceToNextSource(after: message)
    }

    /// 停止播放并结束状态观察。
    public func stop() {
        stopPlaybackAttempt()
        playbackSources = []
        attemptErrors = []
        activeSource = nil
        activeSourceIndex = 0
        sourceCount = 0
        isPlaying = false
        errorMessage = nil
        stopPictureInPicture()
    }

    /// 切换 AVFoundation 播放/暂停状态。
    public func togglePlayback() {
        switch engine {
        case .avFoundation:
            guard avPlayer.currentItem != nil else { return }
            if avPlayer.timeControlStatus == .playing {
                avPlayer.pause()
                isPlaying = false
            } else {
                avPlayer.play()
                isPlaying = true
            }
        case .libMPV:
            libMPVPlayer?.togglePlayback(isPlaying: isPlaying)
            isPlaying.toggle()
            if let session = privatePIPSession {
                ChaosPrivatePIPSetPlaying(session, isPlaying)
            }
        case .webFLV:
            webPlaybackActions?.togglePlayback()
        }
    }

    /// 更新播放器音量；拖动到非零音量时自动解除静音。
    public func setVolume(_ newValue: Float) {
        let clamped = min(max(newValue, 0), 1)
        volume = clamped
        switch engine {
        case .avFoundation:
            avPlayer.volume = clamped
        case .libMPV:
            libMPVPlayer?.setVolume(clamped)
        case .webFLV:
            webPlaybackActions?.setVolume(clamped)
        }
        if clamped > 0, isMuted {
            isMuted = false
            switch engine {
            case .avFoundation:
                avPlayer.isMuted = false
            case .libMPV:
                libMPVPlayer?.setMuted(false)
            case .webFLV:
                webPlaybackActions?.setMuted(false)
            }
        }
    }

    /// 切换静音状态。
    public func toggleMuted() {
        isMuted.toggle()
        switch engine {
        case .avFoundation:
            avPlayer.isMuted = isMuted
        case .libMPV:
            libMPVPlayer?.setMuted(isMuted)
        case .webFLV:
            webPlaybackActions?.setMuted(isMuted)
        }
    }

    func webPlaybackStateChanged(isPlaying: Bool, isMuted: Bool, volume: Float) {
        guard engine == .webFLV else { return }
        self.isPlaying = isPlaying
        self.isMuted = isMuted
        self.volume = min(max(volume, 0), 1)
        if isPlaying {
            confirmPlaybackIfNeeded()
        }
    }

    private func stopPictureInPicture() {
        pictureInPictureStartTask?.cancel()
        pictureInPictureStartTask = nil
        if pictureInPictureController?.isPictureInPictureActive == true {
            pictureInPictureController?.stopPictureInPicture()
        }
        if privatePIPSession != nil {
            closePrivateLibMPVPictureInPicture()
        }
        if appPIPSession != nil {
            closeAppLibMPVPictureInPicture()
        }
        if engine == .webFLV, isPictureInPictureActive {
            webPlaybackActions?.togglePictureInPicture()
        }
        isPictureInPictureActive = false
    }

    private func refreshPictureInPictureAvailability() {
        let isPossible: Bool
        switch engine {
        case .avFoundation:
            isPossible = nativePictureInPicturePossible
        case .libMPV:
            isPossible = libMPVView != nil
        case .webFLV:
            isPossible =
                webPictureInPicturePossible && webPlaybackActions?.togglePictureInPicture != nil
        }
        guard isPictureInPicturePossible != isPossible else { return }
        isPictureInPicturePossible = isPossible
    }

    /// `AVPictureInPictureController` 创建时视频图层通常尚未 ready。
    ///
    /// macOS 对 `pictureInPicturePossible` 的 KVO 通知并非每次都及时送达，
    /// 因此在媒体 ready 后做一段有界重查，避免按钮永久停留在禁用状态。
    private func schedulePictureInPictureAvailabilityChecks(
        for controller: AVPictureInPictureController
    ) {
        pictureInPictureAvailabilityTask?.cancel()
        pictureInPictureAvailabilityTask = Task { [weak self, weak controller] in
            for _ in 0..<20 {
                guard !Task.isCancelled else { return }
                try? await Task.sleep(nanoseconds: 250_000_000)
                guard let self, let controller,
                      self.pictureInPictureController === controller else {
                    return
                }
                self.nativePictureInPicturePossible = controller.isPictureInPicturePossible
                    || self.pictureInPicturePlayerLayer?.isReadyForDisplay == true
                self.refreshPictureInPictureAvailability()
                if controller.isPictureInPicturePossible {
                    return
                }
            }
        }
    }

    private func schedulePictureInPictureStartCheck(
        for controller: AVPictureInPictureController
    ) {
        pictureInPictureStartTask?.cancel()
        pictureInPictureStartTask = Task { [weak self, weak controller] in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled,
                  let self,
                  let controller,
                  self.pictureInPictureController === controller,
                  !controller.isPictureInPictureActive else {
                return
            }
            self.pictureInPictureError = "系统未能启动画中画"
            self.isPictureInPictureActive = false
        }
    }

    fileprivate func nativePictureInPictureDidStart() {
        guard engine == .avFoundation else { return }
        pictureInPictureStartTask?.cancel()
        pictureInPictureStartTask = nil
        isPictureInPictureActive = true
        pictureInPictureError = nil
    }

    fileprivate func nativePictureInPictureDidStop() {
        isPictureInPictureActive = false
    }

    fileprivate func nativePictureInPictureDidFail(_ error: Error) {
        pictureInPictureStartTask?.cancel()
        pictureInPictureStartTask = nil
        isPictureInPictureActive = false
        pictureInPictureError = error.localizedDescription
    }

    private func scheduleOpenTimeout(for currentAttemptID: UUID) {
        openTimeoutTask?.cancel()
        openTimeoutTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 15_000_000_000)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard let self,
                      self.attemptID == currentAttemptID,
                      !self.isPlaying else {
                    return
                }
                self.advanceToNextSource(after: "打开直播流超时")
            }
        }
    }

    private func advanceToNextSource(after message: String) {
        guard !playbackSources.isEmpty else { return }
        openTimeoutTask?.cancel()
        openTimeoutTask = nil
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()
        if privatePIPSession != nil {
            closePrivateLibMPVPictureInPicture()
        }
        libMPVPlayer?.stop()
        webFLVRequest = nil
        isPlaying = false
        attemptErrors.append(message)

        let failedURL = activeSource?.url.absoluteString ?? "-"
        Log.player.error("内置播放线路失败 [\(activeSourceIndex + 1)/\(sourceCount)] \(failedURL)：\(message)")
        activeSourceIndex += 1
        if playbackSources.indices.contains(activeSourceIndex) {
            Log.player.notice("切换到备用播放线路 [\(activeSourceIndex + 1)/\(sourceCount)]")
            playCurrentSource()
        } else {
            finishWithPlaybackError()
        }
    }

    private func finishWithPlaybackError() {
        activeSource = nil
        let last = attemptErrors.last ?? "所有播放线路均不可用"
        errorMessage = sourceCount > 1
            ? "已尝试 \(sourceCount) 条线路，最后错误：\(last)"
            : last
        Log.player.error("内置播放器无法打开当前清晰度：\(self.errorMessage ?? "unknown")")
    }

    private func stopPlaybackAttempt() {
        attemptID = UUID()
        openTimeoutTask?.cancel()
        openTimeoutTask = nil
        pictureInPictureAvailabilityTask?.cancel()
        pictureInPictureAvailabilityTask = nil
        pictureInPictureStartTask?.cancel()
        pictureInPictureStartTask = nil
        if privatePIPSession != nil {
            closePrivateLibMPVPictureInPicture()
        }
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        // 退出页面时保持 currentItem，由对象释放统一结束媒体生命周期。
        avPlayer.pause()
        libMPVPlayer?.stop()
        webFLVRequest = nil
        isPlaying = false
    }

    private func confirmPlaybackIfNeeded() {
        guard confirmedPlaybackSessionID != playbackSessionID else { return }
        confirmedPlaybackSessionID = playbackSessionID
        playbackConfirmation = PlaybackConfirmation(sessionID: playbackSessionID)
    }

    func attachLibMPVView(_ view: LibMPVOpenGLView) {
        libMPVView = view
        libMPVPlayer?.attach(view: view)
        refreshPictureInPictureAvailability()
    }

    func prepareLibMPVRenderContext(for view: LibMPVOpenGLView) -> String? {
        libMPVPlayer?.prepareRenderContext(for: view)
    }

    func renderLibMPV(width: Int32, height: Int32) {
        libMPVPlayer?.render(width: width, height: height)
    }

    func detachLibMPVView(_ view: LibMPVOpenGLView) {
        if libMPVView === view {
            closePrivateLibMPVPictureInPicture()
            closeAppLibMPVPictureInPicture()
            libMPVView = nil
        }
        libMPVPlayer?.destroyRenderContext()
        refreshPictureInPictureAvailability()
    }

    private func togglePrivateLibMPVPictureInPicture() {
        if privatePIPSession != nil || appPIPSession != nil || isPictureInPictureActive {
            Log.player.notice("Private PiP close requested")
            closePrivateLibMPVPictureInPicture()
            closeAppLibMPVPictureInPicture()
            return
        }
        guard let view = libMPVView else {
            pictureInPictureError = "libmpv 渲染视图尚未准备好"
            Log.player.notice("Private PiP unavailable: libmpv view is nil")
            return
        }

        Log.player.notice(
            "Private PiP create requested viewSize=\(view.bounds.width)x\(view.bounds.height) playing=\(isPlaying)"
        )
        let aspectRatio = view.bounds.width > 0 && view.bounds.height > 0
            ? NSSize(width: view.bounds.width, height: view.bounds.height)
            : NSSize(width: 16, height: 9)
        let replacementWindow = view.window
        let replacementRect = view.convert(view.bounds, to: nil)

        if ChaosPrivatePIPIsAvailable() {
            var error: NSString?
            let session = ChaosPrivatePIPCreate(
                view,
                "Chaos Seed",
                aspectRatio,
                isPlaying,
                replacementWindow,
                replacementRect,
                privateLibMPVPIPDidClose,
                Unmanaged.passUnretained(self).toOpaque(),
                &error
            )
            if let session {
                privatePIPSession = session
                isPictureInPictureActive = true
                pictureInPictureError = nil
                view.needsDisplay = true
                Log.player.notice("Private PiP create succeeded")
                refreshPictureInPictureAvailability()
                return
            }

            Log.player.notice(
                "Private PIP.framework create failed, falling back app window: \(String(describing: error))"
            )
        } else {
            Log.player.notice("Private PIP.framework unavailable, using app-owned PiP window")
        }

        guard let session = AppOwnedLibMPVPIPSession(
            videoView: view,
            aspectRatio: aspectRatio,
            onClose: { [weak self] in
                Task { @MainActor [weak self] in
                    self?.appLibMPVPictureInPictureDidClose()
                }
            }
        ) else {
            pictureInPictureError = "无法创建内置 IINA-style 小窗"
            isPictureInPictureActive = false
            Log.player.notice("App-owned PiP window create failed")
            return
        }

        appPIPSession = session
        isPictureInPictureActive = true
        pictureInPictureError = nil
        Log.player.notice("App-owned PiP window create succeeded")
        refreshPictureInPictureAvailability()
    }

    private func closePrivateLibMPVPictureInPicture() {
        guard let session = privatePIPSession else {
            isPictureInPictureActive = false
            return
        }
        ChaosPrivatePIPClose(session)
        if !ChaosPrivatePIPIsActive(session) {
            ChaosPrivatePIPDestroy(session)
            privatePIPSession = nil
        }
        isPictureInPictureActive = false
        libMPVView?.needsDisplay = true
        refreshPictureInPictureAvailability()
    }

    private func closeAppLibMPVPictureInPicture() {
        guard let session = appPIPSession else {
            return
        }
        appPIPSession = nil
        session.close()
        isPictureInPictureActive = false
        libMPVView?.needsDisplay = true
        refreshPictureInPictureAvailability()
    }

    fileprivate func privateLibMPVPictureInPictureDidClose() {
        guard let session = privatePIPSession else {
            isPictureInPictureActive = false
            Log.player.notice("Private PiP didClose without active session")
            return
        }
        privatePIPSession = nil
        isPictureInPictureActive = false
        ChaosPrivatePIPDestroy(session)
        libMPVView?.needsDisplay = true
        Log.player.notice("Private PiP didClose")
        refreshPictureInPictureAvailability()
    }

    private func appLibMPVPictureInPictureDidClose() {
        guard let session = appPIPSession else {
            isPictureInPictureActive = false
            Log.player.notice("App-owned PiP didClose without active session")
            return
        }
        appPIPSession = nil
        session.restore()
        isPictureInPictureActive = false
        libMPVView?.needsDisplay = true
        Log.player.notice("App-owned PiP didClose")
        refreshPictureInPictureAvailability()
    }
}

private final class AppOwnedLibMPVPIPSession: NSObject, NSWindowDelegate {
    private weak var videoView: LibMPVOpenGLView?
    private weak var originalSuperview: NSView?
    private let originalFrame: NSRect
    private let originalAutoresizingMask: NSView.AutoresizingMask
    private let originalSubviewIndex: Int?
    private let onClose: () -> Void
    private var isRestored = false
    private var isClosingProgrammatically = false
    private var window: NSWindow?

    init?(
        videoView: LibMPVOpenGLView,
        aspectRatio: NSSize,
        onClose: @escaping () -> Void
    ) {
        guard let originalSuperview = videoView.superview else { return nil }
        self.videoView = videoView
        self.originalSuperview = originalSuperview
        self.originalFrame = videoView.frame
        self.originalAutoresizingMask = videoView.autoresizingMask
        self.originalSubviewIndex = originalSuperview.subviews.firstIndex(of: videoView)
        self.onClose = onClose
        super.init()

        let currentWindowFrame = originalSuperview.window?.frame
            ?? NSScreen.main?.visibleFrame
            ?? NSRect(x: 120, y: 120, width: 1280, height: 720)
        let ratio = aspectRatio.width > 0 && aspectRatio.height > 0
            ? aspectRatio.width / aspectRatio.height
            : 16 / 9
        let width = min(520, max(360, currentWindowFrame.width * 0.34))
        let height = width / ratio
        let origin = NSPoint(
            x: currentWindowFrame.maxX - width - 36,
            y: currentWindowFrame.maxY - height - 72
        )
        let frame = NSRect(origin: origin, size: NSSize(width: width, height: height))

        let container = NSView(frame: NSRect(origin: .zero, size: frame.size))
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.black.cgColor
        videoView.removeFromSuperview()
        videoView.frame = container.bounds
        videoView.autoresizingMask = [.width, .height]
        container.addSubview(videoView)

        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Chaos Seed"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        window.level = .floating
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.minSize = NSSize(width: 280, height: 160)
        window.aspectRatio = aspectRatio.width > 0 && aspectRatio.height > 0
            ? aspectRatio
            : NSSize(width: 16, height: 9)
        window.backgroundColor = .black
        window.contentView = container
        window.delegate = self
        window.orderFrontRegardless()
        self.window = window
    }

    func close() {
        isClosingProgrammatically = true
        window?.close()
        restore()
        isClosingProgrammatically = false
    }

    func restore() {
        guard !isRestored,
              let videoView,
              let originalSuperview else {
            return
        }
        videoView.removeFromSuperview()
        videoView.frame = originalFrame
        videoView.autoresizingMask = originalAutoresizingMask
        if let originalSubviewIndex,
           originalSubviewIndex <= originalSuperview.subviews.count {
            originalSuperview.addSubview(
                videoView,
                positioned: .below,
                relativeTo: originalSubviewIndex < originalSuperview.subviews.count
                    ? originalSuperview.subviews[originalSubviewIndex]
                    : nil
            )
        } else {
            originalSuperview.addSubview(videoView)
        }
        videoView.needsDisplay = true
        isRestored = true
    }

    func windowWillClose(_ notification: Notification) {
        restore()
        if !isClosingProgrammatically {
            onClose()
        }
    }
}

private let privateLibMPVPIPDidClose: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { context in
    guard let context else { return }
    let player = Unmanaged<BuiltinPlayer>
        .fromOpaque(context)
        .takeUnretainedValue()
    Task { @MainActor in
        player.privateLibMPVPictureInPictureDidClose()
    }
}

struct WebPlaybackActions {
    let togglePlayback: () -> Void
    let setMuted: (Bool) -> Void
    let setVolume: (Float) -> Void
    let togglePictureInPicture: () -> Void
}

private final class BuiltinPictureInPictureDelegate:
    NSObject,
    AVPictureInPictureControllerDelegate
{
    weak var player: BuiltinPlayer?

    init(player: BuiltinPlayer) {
        self.player = player
    }

    func pictureInPictureControllerDidStartPictureInPicture(
        _ pictureInPictureController: AVPictureInPictureController
    ) {
        Task { @MainActor [weak self] in
            self?.player?.nativePictureInPictureDidStart()
        }
    }

    func pictureInPictureControllerDidStopPictureInPicture(
        _ pictureInPictureController: AVPictureInPictureController
    ) {
        Task { @MainActor [weak self] in
            self?.player?.nativePictureInPictureDidStop()
        }
    }

    func pictureInPictureController(
        _ pictureInPictureController: AVPictureInPictureController,
        failedToStartPictureInPictureWithError error: any Error
    ) {
        Task { @MainActor [weak self] in
            self?.player?.nativePictureInPictureDidFail(error)
        }
    }
}

/// 一次 HTTP-FLV 播放请求；新 UUID 用于触发 WebKit 可靠重载。
public struct WebFLVPlaybackRequest: Identifiable, Equatable {
    public let id = UUID()
    public let url: URL
    public let headers: [String: String]

    public init(url: URL, headers: [String: String]) {
        self.url = url
        self.headers = headers
    }
}

// MARK: - HDR / EDR 窗口配置

extension BuiltinPlayer {
    /// 为播放窗口配置 EDR 支持的 colorspace。
    ///
    /// 在 `NSWindow` 出现后调用，使视频渲染管线正确输出 HDR。
    /// - 使用 `CGColorSpace.displayP3_HLG` 以在 SDR 内容上兼容；
    /// 具体 HDR 格式是否生效仍由 AVPlayer 与媒体元数据判断。
    public static func configureWindowForHDR(_ window: NSWindow?) {
        guard let window else { return }
        // 窗口需 EDR（Extended Dynamic Range）才能渲染 HDR 内容。
        if let screen = window.screen {
            let maxEDR = screen.maximumExtendedDynamicRangeColorComponentValue
            // 仅在实际支持 EDR 的设备上开启，避免 SDR 屏幕亮度异常。
            if maxEDR > 1.0 {
                window.colorSpace = screen.colorSpace
            }
        }
        // 视频图层配置：AVPlayerLayer 自动选择 EDR（如有）。
        // 不需要手动设置 pixelFormat，系统会根据窗口 capabilities 自动启用。
    }
}
