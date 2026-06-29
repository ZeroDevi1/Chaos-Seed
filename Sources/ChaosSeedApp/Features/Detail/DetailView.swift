import SwiftUI
import SwiftData
import AppKit

enum PlayerPresentationPolicy {
    static func showsPageChrome(isFullScreen: Bool) -> Bool {
        !isFullScreen
    }

    static func rightRailOnEnteringFullScreen() -> Bool {
        false
    }
}

/// 直播间详情页：清晰度/线路列表 + 内置播放器 / IINA 外部播放器。
///
/// 复刻原型 `detail.jsx`，升级后支持：
/// - 内置 AVPlayer 播放器（`BuiltinPlayer`），直接在应用内渲染视频；
/// - IINA 外部播放器（`IINALauncher`），通过系统调用拉起；
/// - 设置页可切换默认播放器偏好。
public struct DetailView: View {
    @EnvironmentObject private var appState: AppState
    @Environment(\.modelContext) private var modelContext
    @AppStorage("debugLogging") private var debugLogging = false
    @AppStorage("experimentalSystemPiPForFLV") private var experimentalSystemPiPForFLV = false
    @AppStorage("danmakuRightRailExpanded") private var isDanmakuRailExpanded = true
    @StateObject private var vm: DetailViewModel
    @State private var showDanmakuSettings = false
    @State private var showQualityPicker = false
    @State private var isFullScreen = false
    @State private var railExpandedBeforeFullScreen = true
    @State private var controlsVisible = true
    @State private var controlHideTask: Task<Void, Never>?
    @State private var windowChromeSnapshot: WindowChromeSnapshot?
    let onBack: () -> Void

    public init(liveKit: LiveKit, room: LiveRoomCard, onBack: @escaping () -> Void) {
        self._vm = StateObject(wrappedValue: DetailViewModel(room: room, liveKit: liveKit))
        self.onBack = onBack
    }

    public var body: some View {
        VStack(spacing: 0) {
            if PlayerPresentationPolicy.showsPageChrome(isFullScreen: isFullScreen) {
                toolbar
                Divider()
            }
            // 当内置播放器激活时，用内置播放器替换整个内容区。
            if let player = vm.builtinPlayer {
                builtinPlayerContent(player: player)
            } else {
                qualityListContent
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .background(Color(nsColor: .windowBackgroundColor).ignoresSafeArea())
        .ignoresSafeArea(.container, edges: isFullScreen ? .all : [])
        .onAppear {
            vm.decode()
            DispatchQueue.main.async {
                if NSApp.keyWindow?.styleMask.contains(.fullScreen) == true {
                    enterImmersiveFullScreen()
                }
            }
        }
        .onDisappear {
            controlHideTask?.cancel()
            restoreWindowChrome()
            vm.stopBuiltinPlayer()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didEnterFullScreenNotification)) { _ in
            enterImmersiveFullScreen()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didExitFullScreenNotification)) { _ in
            exitImmersiveFullScreen()
        }
    }

    // MARK: - 工具栏

    private var toolbar: some View {
        HStack(spacing: 12) {
            Button(action: {
                vm.stopBuiltinPlayer()
                onBack()
            }) {
                Label("返回首页", systemImage: "chevron.left")
            }
            Spacer()
            Text(vm.builtinPlayer != nil ? vm.room.title : "直播间详情")
                .font(.system(size: 16, weight: .bold))
                .lineLimit(1)
            Spacer()
            // 工具栏右侧操作区：切换播放器 / 停止内置播放器。
            if vm.builtinPlayer != nil {
                Button {
                    vm.play(using: .iina, onResult: handlePlaybackResult)
                } label: {
                    Label("IINA", systemImage: "arrow.up.forward.app")
                }
                Button {
                    vm.stopBuiltinPlayer()
                } label: {
                    Label("关闭播放器", systemImage: "xmark.circle")
                }
                .padding(.trailing, 8)
            }
            Spacer().frame(width: 100) // 平衡标题居中
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .liquidGlassBar()
    }

    // MARK: - 内置播放器内容区

    private func builtinPlayerContent(player: BuiltinPlayer) -> some View {
        GeometryReader { geo in
            let railWidth = DanmakuPresentationPolicy.rightRailWidth(
                totalWidth: geo.size.width,
                isExpanded: isDanmakuRailExpanded
            )
            let videoWidth = DanmakuPresentationPolicy.videoWidth(
                totalWidth: geo.size.width,
                isRightRailExpanded: isDanmakuRailExpanded
            )
            let interactionWidth = max(0, videoWidth - 44)

            ZStack(alignment: .bottomLeading) {
                playerSurface(player: player)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(Color.black)

                // NSOpenGLView / WebView 会截获 SwiftUI hover；单独铺一层 AppKit 事件层，
                // 只覆盖视频区域，让控制条回显不依赖底层渲染视图是否传递鼠标事件。
                PlayerMouseInteractionLayer(
                    onActivity: {
                        revealControls(for: player)
                    },
                    onDoubleClick: {
                        toggleFullScreen()
                    },
                    onExit: {
                        scheduleControlHide(for: player)
                    },
                    onKeyDown: { event in
                        handlePlayerKeyDown(event, player: player)
                    }
                )
                .frame(width: interactionWidth, height: max(0, geo.size.height - 96))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .onAppear {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                        BuiltinPlayer.configureWindowForHDR(NSApp.keyWindow)
                    }
                    revealControls(for: player)
                }

                if controlsVisible || !player.isPlaying || player.errorMessage != nil {
                    PlayerControlHUD(
                        player: player,
                        showOverlayDanmaku: $vm.showOverlayDanmaku,
                        isDanmakuRailExpanded: $isDanmakuRailExpanded,
                        showDanmakuSettings: $showDanmakuSettings,
                        showQualityPicker: $showQualityPicker,
                        selectedQualityLabel: selectedQualityLabel,
                        isFullScreen: isFullScreen,
                        experimentalSystemPiPForFLV: experimentalSystemPiPForFLV,
                        availableWidth: videoWidth,
                        danmakuStatus: danmakuStatusText,
                        onSelectQuality: { variantID in
                            vm.selectedVariantId = variantID
                            vm.play(onResult: handlePlaybackResult)
                            revealControls(for: player)
                        },
                        variants: vm.variants,
                        selectedVariantID: vm.selectedVariantId,
                        danmakuConfig: $vm.danmakuConfig,
                        onTogglePictureInPicture: {
                            togglePictureInPicture(player)
                        },
                        onToggleFullScreen: toggleFullScreen
                    )
                    .padding(.horizontal, 22)
                    .padding(.bottom, isFullScreen ? 22 : 16)
                    .frame(width: videoWidth, alignment: .bottom)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .background(Color.black)
            .animation(.easeOut(duration: 0.18), value: controlsVisible)
            .animation(.easeInOut(duration: 0.2), value: railWidth)
        }
        .onChange(of: player.playbackConfirmation) { _, confirmation in
            guard confirmation != nil else { return }
            recordHistory(method: .builtin)
        }
        .onChange(of: player.pictureInPictureError) { _, message in
            if let message {
                appState.showToast(message)
            }
        }
    }

    @ViewBuilder
    private func playerSurface(player: BuiltinPlayer) -> some View {
        if let danmakuClient = vm.danmakuClient {
            ObservedDanmakuPlayer(
                player: player,
                client: danmakuClient,
                config: vm.danmakuConfig,
                showOverlayDanmaku: vm.showOverlayDanmaku,
                isRightRailExpanded: $isDanmakuRailExpanded
            )
        } else {
            BuiltinPlayerWithDanmaku(
                player: player,
                danmakuComments: [],
                danmakuConfig: vm.danmakuConfig,
                showOverlayDanmaku: vm.showOverlayDanmaku,
                isRightRailExpanded: $isDanmakuRailExpanded
            )
        }
    }

    private var danmakuStatusText: String {
        if let client = vm.danmakuClient {
            if client.isConnected { return "弹幕已连接" }
            if client.isReconnecting { return "弹幕重连中" }
            if client.error != nil { return "弹幕连接失败" }
        }
        return vm.danmakuConnectionError == nil ? "弹幕连接中" : "弹幕连接失败"
    }

    private var selectedQualityLabel: String {
        vm.variants.first(where: { $0.id == vm.selectedVariantId })?.label ?? "清晰度"
    }

    private func revealControls(for player: BuiltinPlayer) {
        controlsVisible = true
        scheduleControlHide(for: player)
    }

    private func scheduleControlHide(for player: BuiltinPlayer) {
        controlHideTask?.cancel()
        guard player.isPlaying,
              player.errorMessage == nil,
              !showDanmakuSettings,
              !showQualityPicker else {
            return
        }
        controlHideTask = Task {
            try? await Task.sleep(for: .seconds(2.5))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                controlsVisible = false
            }
        }
    }

    private func toggleFullScreen() {
        NSApp.keyWindow?.toggleFullScreen(nil)
    }

    private func enterImmersiveFullScreen() {
        railExpandedBeforeFullScreen = isDanmakuRailExpanded
        isDanmakuRailExpanded = PlayerPresentationPolicy.rightRailOnEnteringFullScreen()
        isFullScreen = true
        hideWindowChrome()
        controlsVisible = false
    }

    private func exitImmersiveFullScreen() {
        restoreWindowChrome()
        isFullScreen = false
        isDanmakuRailExpanded = railExpandedBeforeFullScreen
        controlsVisible = true
    }

    private func hideWindowChrome() {
        guard let window = NSApp.keyWindow else { return }
        if windowChromeSnapshot == nil {
            windowChromeSnapshot = WindowChromeSnapshot(window: window)
        }
        window.toolbar?.isVisible = false
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.styleMask.insert(.fullSizeContentView)
    }

    private func restoreWindowChrome() {
        guard let snapshot = windowChromeSnapshot,
              let window = NSApp.keyWindow else {
            return
        }
        snapshot.restore(to: window)
        windowChromeSnapshot = nil
    }

    private func togglePictureInPicture(_ player: BuiltinPlayer) {
        Log.player.notice(
            "PiP requested engine=\(String(describing: player.engine)) experimental=\(experimentalSystemPiPForFLV)"
        )
        if player.engine == .libMPV || player.engine == .webFLV {
            if experimentalSystemPiPForFLV {
                if player.isPictureInPictureActive {
                    Log.player.notice("PiP stopping active session")
                    player.togglePictureInPicture()
                    return
                }
                if player.engine == .webFLV, player.isPictureInPicturePossible {
                    Log.player.notice("PiP trying Web video element")
                    player.togglePictureInPicture()
                    return
                }
                if player.engine == .libMPV {
                    appState.showToast("正在打开内置 IINA-style 小窗")
                    Log.player.notice("PiP trying private libmpv bridge")
                    player.togglePictureInPicture()
                    if player.isPictureInPictureActive {
                        Log.player.notice("PiP private libmpv bridge started")
                        appState.showToast("已打开内置 IINA-style 小窗")
                        return
                    }
                }
                let reason = player.pictureInPictureError
                    ?? "当前 HTTP-FLV 后端无法交给系统画中画"
                Log.player.notice("PiP private path unavailable: \(reason)")
                appState.showToast("\(reason)，已回退 IINA 小窗")
            }
            Log.player.notice("PiP falling back to IINA")
            let result = vm.openActiveBuiltinSourceInIINA(startPictureInPicture: true)
            switch result {
            case .iina:
                appState.showToast("已通过 IINA 小窗打开「\(vm.room.title)」")
            case .iinaNotFound:
                appState.showToast("未检测到 IINA，请检查设置中的应用路径")
            case .failure(let message):
                appState.showToast(message)
            case .builtin, .unsupportedBuiltin:
                break
            }
            return
        }

        Log.player.notice("PiP trying native AVPictureInPictureController")
        player.togglePictureInPicture()
        if let message = player.pictureInPictureError {
            appState.showToast(message)
        } else if !player.isPictureInPicturePossible && !player.isPictureInPictureActive {
            appState.showToast("系统画中画尚未准备好，请稍后重试")
        }
    }

    private func handlePlayerKeyDown(
        _ event: NSEvent,
        player: BuiltinPlayer
    ) -> Bool {
        switch event.charactersIgnoringModifiers?.lowercased() {
        case " ":
            player.togglePlayback()
        case "f":
            toggleFullScreen()
        case "m":
            player.toggleMuted()
        default:
            if event.keyCode == 53, isFullScreen {
                toggleFullScreen()
            } else {
                return false
            }
        }
        revealControls(for: player)
        return true
    }

    // MARK: - 清晰度列表内容区（未播放时）

    private var qualityListContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                qualitySection
                if debugLogging {
                    logBox
                }
            }
            .padding(20)
        }
    }

    // MARK: - 头部信息

    private var header: some View {
        HStack(alignment: .top, spacing: 20) {
            CoverImage(cover: vm.room.cover, site: vm.room.site, placeholderIcon: "play.tv.fill")
                .frame(width: 280, height: 158)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 10) {
                Text(vm.room.title)
                    .font(.system(size: 22, weight: .bold))
                    .textSelection(.enabled)
                HStack(spacing: 8) {
                    PlatformBadge(site: vm.room.site)
                    Text("主播：\(vm.room.userName ?? "未知")")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                    if let online = vm.room.online {
                        Text("· 在线：\(formatViewers(online))")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer(minLength: 8)
                HStack(spacing: 10) {
                    // 播放按钮：根据 playerPreference 决定标签文字。
                    Button {
                        vm.play(onResult: handlePlaybackResult)
                    } label: {
                        let label = vm.playerPreference == .iina ? "在 IINA 中播放" : "播放"
                        Label(label, systemImage: "play.fill")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(vm.selectedVariantId == nil || vm.loading)

                    if vm.playerPreference == .builtin {
                        Button {
                            vm.play(using: .iina, onResult: handlePlaybackResult)
                        } label: {
                            Label("IINA", systemImage: "arrow.up.forward.app")
                        }
                        .buttonStyle(.bordered)
                        .disabled(vm.selectedVariantId == nil || vm.loading)
                    }

                    // 复制 URL 按钮。
                    Button {
                        if let url = vm.copySelectedUrl() {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(url, forType: .string)
                            appState.showToast("已复制直连 URL 到剪贴板")
                        } else {
                            appState.showToast("当前清晰度尚未解析出直连 URL")
                        }
                    } label: {
                        Label("复制 URL", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.bordered)
                    .disabled(vm.selectedVariantId == nil)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    // MARK: - 清晰度列表

    private var qualitySection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("清晰度 / 线路")
                .font(.system(size: 12, weight: .bold))
                .textCase(.uppercase)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(Color.gray.opacity(0.06))
            if vm.loading {
                ProgressView()
                    .controlSize(.regular)
                    .frame(maxWidth: .infinity, minHeight: 120)
            } else {
                ForEach(vm.variants) { variant in
                    let isSelected = vm.selectedVariantId == variant.id
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(variant.label)
                                .font(.system(size: 13, weight: .semibold))
                            Text(lineName(for: variant))
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(variantStatus(variant))
                            .font(.system(size: 11, weight: .semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(
                                variant.builtinPlaybackSource != nil
                                ? Color.green.opacity(0.16)
                                : Color.gray.opacity(0.16),
                                in: Capsule()
                            )
                            .foregroundStyle(
                                variant.builtinPlaybackSource != nil
                                ? Color(red: 0.12, green: 0.54, blue: 0.18)
                                : .secondary
                            )
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                    .background(isSelected ? Color.accentColor.opacity(0.08) : .clear)
                    .contentShape(Rectangle())
                    .onTapGesture { vm.selectedVariantId = variant.id }
                    if variant.id != vm.variants.last?.id {
                        Divider()
                    }
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(.separator)
        )
        .liquidGlassBackground(in: .rect(cornerRadius: 10))
    }

    /// 用 quality 值映射到「线路 N」，便于复刻原型观感。
    private func lineName(for variant: StreamVariant) -> String {
        "线路 \((variant.quality % 3) + 1)"
    }

    private func variantStatus(_ variant: StreamVariant) -> String {
        if let source = variant.builtinPlaybackSource {
            switch source.engine {
            case .avFoundation:
                return "内置 HLS"
            case .libMPV:
                return "内置 libmpv"
            case .webFLV:
                return "内置 FLV"
            }
        }
        if variant.isResolved {
            return "FLV / IINA"
        }
        return "需二段解析"
    }

    // MARK: - 日志框

    private var logBox: some View {
        Text(vm.logs.isEmpty ? "（无日志）" : vm.logs)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
            .padding(12)
            .background(Color.gray.opacity(0.08), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func handlePlaybackResult(_ result: PlaybackLaunchResult) {
        switch result {
        case .builtin:
            break
        case .iina:
            recordHistory(method: .iina)
            appState.showToast("已通过 IINA 打开「\(vm.room.title)」")
        case .iinaNotFound:
            appState.showToast("未检测到 IINA，请检查设置中的应用路径")
        case .unsupportedBuiltin:
            appState.showToast("当前线路仅提供不受支持的 P2P 地址，请切换 IINA")
        case .failure(let message):
            appState.showToast(message)
        }
    }

    private func recordHistory(method: PlaybackMethod) {
        do {
            try PlaybackHistoryStore.record(
                room: vm.room,
                method: method,
                qualityLabel: selectedQualityLabel,
                in: modelContext
            )
        } catch {
            Log.app.error("保存播放历史失败", error: error)
        }
    }
}

private struct ObservedDanmakuPlayer: View {
    @ObservedObject var player: BuiltinPlayer
    @ObservedObject var client: DanmakuClient
    let config: DanmakuConfig
    let showOverlayDanmaku: Bool
    @Binding var isRightRailExpanded: Bool

    var body: some View {
        BuiltinPlayerWithDanmaku(
            player: player,
            danmakuComments: client.comments,
            danmakuConfig: config,
            showOverlayDanmaku: showOverlayDanmaku,
            isRightRailExpanded: $isRightRailExpanded
        )
    }
}

private struct ObservedPlaybackStatus: View {
    @ObservedObject var player: BuiltinPlayer

    var body: some View {
        HStack(spacing: 12) {
            HStack(spacing: 6) {
                Circle()
                    .fill(player.isPlaying ? Color(red: 0.98, green: 0.32, blue: 0.53) : .secondary)
                    .frame(width: 7, height: 7)
                Text(player.isPlaying ? "直播" : "加载中")
                    .font(.system(size: 12, weight: .semibold))
            }

            if player.sourceCount > 1 {
                Text("线路 \(min(player.activeSourceIndex + 1, player.sourceCount))/\(player.sourceCount)")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }
        }
        .lineLimit(1)
        .fixedSize(horizontal: true, vertical: false)
    }
}

private struct PlayerControlHUD: View {
    @ObservedObject var player: BuiltinPlayer
    @Binding var showOverlayDanmaku: Bool
    @Binding var isDanmakuRailExpanded: Bool
    @Binding var showDanmakuSettings: Bool
    @Binding var showQualityPicker: Bool
    let selectedQualityLabel: String
    let isFullScreen: Bool
    let experimentalSystemPiPForFLV: Bool
    let availableWidth: CGFloat
    let danmakuStatus: String
    let onSelectQuality: (String) -> Void
    let variants: [StreamVariant]
    let selectedVariantID: String?
    @Binding var danmakuConfig: DanmakuConfig
    let onTogglePictureInPicture: () -> Void
    let onToggleFullScreen: () -> Void

    var body: some View {
        HStack(spacing: isCompact ? 10 : 14) {
            ObservedPlaybackStatus(player: player)

            Divider()
                .frame(height: 20)
                .overlay(Color.white.opacity(0.24))

            controlButton(
                systemImage: player.isPlaying ? "pause.fill" : "play.fill",
                help: player.isPlaying ? "暂停（空格）" : "播放（空格）",
                action: player.togglePlayback
            )

            controlButton(
                systemImage: player.isMuted || player.volume == 0
                    ? "speaker.slash.fill"
                    : "speaker.wave.2.fill",
                help: player.isMuted ? "取消静音（M）" : "静音（M）",
                action: player.toggleMuted
            )

            Slider(
                value: Binding(
                    get: { Double(player.volume) },
                    set: { player.setVolume(Float($0)) }
                ),
                in: 0...1
            )
            .frame(width: isCompact ? 88 : 118)
            .tint(.white)
            .accessibilityLabel("音量")

            Spacer(minLength: isCompact ? 8 : 18)

            if !isCompact {
                Text(danmakuStatus)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.white.opacity(0.72))
                    .lineLimit(1)
            }

            controlButton(
                systemImage: showOverlayDanmaku ? "text.bubble.fill" : "text.bubble",
                help: showOverlayDanmaku ? "关闭画面弹幕" : "开启画面弹幕"
            ) {
                showOverlayDanmaku.toggle()
            }

            controlButton(
                systemImage: isDanmakuRailExpanded ? "sidebar.right" : "sidebar.trailing",
                help: isDanmakuRailExpanded ? "收起弹幕列表" : "展开弹幕列表"
            ) {
                isDanmakuRailExpanded.toggle()
            }

            controlButton(systemImage: "slider.horizontal.3", help: "弹幕设置") {
                showDanmakuSettings.toggle()
            }
            .popover(isPresented: $showDanmakuSettings, arrowEdge: .bottom) {
                DanmakuSettingsPanel(
                    config: $danmakuConfig,
                    onClose: { showDanmakuSettings = false }
                )
                .padding(8)
            }

            Button {
                showQualityPicker.toggle()
            } label: {
                HStack(spacing: 5) {
                    Text(selectedQualityLabel)
                        .lineLimit(1)
                    Image(systemName: "chevron.up.chevron.down")
                        .font(.system(size: 9, weight: .semibold))
                }
                .font(.system(size: 12, weight: .semibold))
                .frame(minWidth: 64, minHeight: 32)
                .padding(.horizontal, 5)
            }
            .buttonStyle(.plain)
            .help("切换清晰度")
            .popover(isPresented: $showQualityPicker, arrowEdge: .bottom) {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(variants) { variant in
                        Button {
                            showQualityPicker = false
                            onSelectQuality(variant.id)
                        } label: {
                            HStack {
                                Text(variant.label)
                                Spacer()
                                if selectedVariantID == variant.id {
                                    Image(systemName: "checkmark")
                                }
                            }
                            .frame(minWidth: 150)
                        }
                        .buttonStyle(.plain)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                    }
                }
                .padding(8)
            }

            controlButton(
                systemImage: player.isPictureInPictureActive ? "pip.exit" : "pip.enter",
                help: pictureInPictureHelp
            ) {
                onTogglePictureInPicture()
            }

            controlButton(
                systemImage: isFullScreen
                    ? "arrow.down.right.and.arrow.up.left"
                    : "arrow.up.left.and.arrow.down.right",
                help: isFullScreen ? "退出全屏（Esc）" : "进入全屏（F）",
                action: onToggleFullScreen
            )
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .foregroundStyle(.white)
        .background(.black.opacity(0.72), in: Capsule())
        .overlay(
            Capsule()
                .strokeBorder(Color.white.opacity(0.12))
        )
        .shadow(color: .black.opacity(0.38), radius: 18, y: 7)
    }

    private var isCompact: Bool {
        availableWidth < 760
    }

    private var pictureInPictureHelp: String {
        if let error = player.pictureInPictureError {
            return error
        }
        if player.isPictureInPictureActive {
            return "退出系统画中画"
        }
        if player.engine == .libMPV || player.engine == .webFLV {
            if experimentalSystemPiPForFLV {
                return player.engine == .libMPV
                    ? "打开内置 IINA-style 小窗"
                    : "实验系统画中画；不可用时回退 IINA 小窗"
            }
            return "使用 IINA 小窗播放当前 HTTP-FLV"
        }
        return player.isPictureInPicturePossible ? "进入系统画中画" : "系统画中画暂不可用"
    }

    private func controlButton(
        systemImage: String,
        help: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 14, weight: .semibold))
                .frame(width: 32, height: 32)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(help)
    }
}

private struct PlayerMouseInteractionLayer: NSViewRepresentable {
    let onActivity: () -> Void
    let onDoubleClick: () -> Void
    let onExit: () -> Void
    let onKeyDown: (NSEvent) -> Bool

    func makeNSView(context: Context) -> TrackingView {
        let view = TrackingView()
        view.onActivity = onActivity
        view.onDoubleClick = onDoubleClick
        view.onExit = onExit
        view.onKeyDown = onKeyDown
        return view
    }

    func updateNSView(_ nsView: TrackingView, context: Context) {
        nsView.onActivity = onActivity
        nsView.onDoubleClick = onDoubleClick
        nsView.onExit = onExit
        nsView.onKeyDown = onKeyDown
    }

    final class TrackingView: NSView {
        var onActivity: () -> Void = {}
        var onDoubleClick: () -> Void = {}
        var onExit: () -> Void = {}
        var onKeyDown: (NSEvent) -> Bool = { _ in false }
        private var trackingArea: NSTrackingArea?

        override var acceptsFirstResponder: Bool { true }

        override var focusRingType: NSFocusRingType {
            get { .none }
            set {}
        }

        override func updateTrackingAreas() {
            super.updateTrackingAreas()
            if let trackingArea {
                removeTrackingArea(trackingArea)
            }
            let options: NSTrackingArea.Options = [
                .activeInKeyWindow,
                .mouseEnteredAndExited,
                .mouseMoved,
                .inVisibleRect
            ]
            let area = NSTrackingArea(
                rect: bounds,
                options: options,
                owner: self,
                userInfo: nil
            )
            addTrackingArea(area)
            trackingArea = area
        }

        override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
            true
        }

        override func mouseEntered(with event: NSEvent) {
            onActivity()
        }

        override func mouseMoved(with event: NSEvent) {
            onActivity()
        }

        override func mouseExited(with event: NSEvent) {
            onExit()
        }

        override func mouseDown(with event: NSEvent) {
            window?.makeFirstResponder(self)
            onActivity()
            if event.clickCount >= 2 {
                onDoubleClick()
            }
        }

        override func keyDown(with event: NSEvent) {
            if !onKeyDown(event) {
                super.keyDown(with: event)
            }
        }
    }
}

private struct WindowChromeSnapshot {
    let toolbarWasVisible: Bool?
    let titleVisibility: NSWindow.TitleVisibility
    let titlebarAppearsTransparent: Bool
    let hadFullSizeContentView: Bool

    init(window: NSWindow) {
        toolbarWasVisible = window.toolbar?.isVisible
        titleVisibility = window.titleVisibility
        titlebarAppearsTransparent = window.titlebarAppearsTransparent
        hadFullSizeContentView = window.styleMask.contains(.fullSizeContentView)
    }

    func restore(to window: NSWindow) {
        if let toolbarWasVisible {
            window.toolbar?.isVisible = toolbarWasVisible
        }
        window.titleVisibility = titleVisibility
        window.titlebarAppearsTransparent = titlebarAppearsTransparent
        if !hadFullSizeContentView {
            window.styleMask.remove(.fullSizeContentView)
        }
    }
}
