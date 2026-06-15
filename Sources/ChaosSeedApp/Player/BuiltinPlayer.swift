import SwiftUI
import AVKit
import AppKit

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

/// 内置播放器：HLS / MP4 使用 AVPlayer，HTTP-FLV 使用 WebKit + MSE。
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
    /// 底层播放器在视图生命周期内保持同一实例，避免 AVPlayerView 更新期间解绑导致 AVKit 崩溃。
    public let avPlayer = AVPlayer()
    @Published public var isPlaying = false
    @Published public var errorMessage: String?
    @Published public private(set) var engine: BuiltinPlaybackEngine = .avFoundation
    @Published public private(set) var webFLVRequest: WebFLVPlaybackRequest?
    private var itemStatusObservation: NSKeyValueObservation?

    public var playerView: AnyView? {
        AnyView(BuiltinPlayerView(player: self))
    }

    /// 启动指定后端的内置播放。
    public func play(source: BuiltinPlaybackSource, hints: PlaybackHints) {
        switch source.engine {
        case .avFoundation:
            playAVFoundation(url: source.url, hints: hints)
        case .webFLV:
            playWebFLV(url: source.url, hints: hints)
        }
    }

    /// `LivePlayer` 兼容入口，明确按 AVFoundation 媒体处理。
    public func play(url: URL, hints: PlaybackHints) {
        play(source: BuiltinPlaybackSource(url: url, engine: .avFoundation), hints: hints)
    }

    /// 启动 AVFoundation 播放：设置 HTTP header 并开始拉流。
    ///
    /// 系统媒体能力：
    /// - E-AC-3 / Atmos 是否输出取决于流本身、系统解码器与当前音频设备；
    /// - HDR / Dolby Vision 是否呈现取决于视频元数据、显示器和 AVPlayer 渲染链。
    private func playAVFoundation(url: URL, hints: PlaybackHints) {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()
        engine = .avFoundation
        webFLVRequest = nil

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
        avPlayer.volume = 1.0
        avPlayer.replaceCurrentItem(with: playerItem)

        self.errorMessage = nil
        self.isPlaying = false

        itemStatusObservation = playerItem.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
            Task { @MainActor in
                guard let self else { return }
                switch item.status {
                case .readyToPlay:
                    self.isPlaying = true
                case .failed:
                    self.isPlaying = false
                    self.errorMessage = item.error?.localizedDescription ?? "播放器无法读取当前直播流"
                    Log.player.error("AVPlayerItem 加载失败：\(self.errorMessage ?? "unknown")")
                case .unknown:
                    break
                @unknown default:
                    break
                }
            }
        }

        avPlayer.play()
    }

    /// 启动应用内 HTTP-FLV 播放，由 WebKit 中的 mpegts.js 转封装到 MSE。
    private func playWebFLV(url: URL, hints: PlaybackHints) {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        avPlayer.pause()

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
    }

    func webFLVDidStartPlaying() {
        guard engine == .webFLV else { return }
        isPlaying = true
        errorMessage = nil
    }

    func webFLVDidPause() {
        guard engine == .webFLV else { return }
        isPlaying = false
    }

    func webFLVDidFail(_ message: String) {
        guard engine == .webFLV else { return }
        isPlaying = false
        errorMessage = message.isEmpty ? "HTTP-FLV 播放失败" : message
        Log.player.error("WebFLV 播放失败：\(self.errorMessage ?? "unknown")")
    }

    /// 停止播放并结束状态观察。
    public func stop() {
        itemStatusObservation?.invalidate()
        itemStatusObservation = nil
        // 退出页面时不把 AVPlayerView.player 或 currentItem 置空。AVKit 在 macOS 27 Beta
        // 的控件更新事务中解绑播放器会触发主线程 SIGTRAP；对象随视图模型一起释放即可。
        avPlayer.pause()
        webFLVRequest = nil
        isPlaying = false
        errorMessage = nil
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
