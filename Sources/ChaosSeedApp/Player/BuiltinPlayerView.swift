import SwiftUI
import AVKit

/// 内置播放器的 SwiftUI 视图：将 AVPlayerView 嵌入 SwiftUI 布局，并叠加弹幕层。
///
/// 特性：
/// - 自动绑定 `BuiltinPlayer.avPlayer` 到视频渲染层。
/// - 支持全屏 / 画中画（使用 macOS 26 的 `AVPlayerView` 原生控件）。
/// - HDR（EDR）由视频解码管线自动启用。
/// - 弹幕 Canvas 叠加层（通过 `danmakuComments` + `danmakuConfig` 注入）。
///
/// 用法：
/// ```swift
/// BuiltinPlayerView(player: player, danmakuComments: $vm.comments, danmakuConfig: $vm.danmakuConfig)
///     .onAppear { BuiltinPlayer.configureWindowForHDR(NSApp.keyWindow) }
/// ```
public struct BuiltinPlayerView: NSViewRepresentable {
    let player: BuiltinPlayer

    /// 弹幕数据源（从 DanmakuClient 同步接收）。
    var danmakuComments: [DanmakuComment] = []
    /// 弹幕显示配置。
    var danmakuConfig: DanmakuConfig = .default
    /// 是否显示弹幕。
    var showDanmaku: Bool = false

    public init(player: BuiltinPlayer,
                danmakuComments: [DanmakuComment] = [],
                danmakuConfig: DanmakuConfig = .default,
                showDanmaku: Bool = false) {
        self.player = player
        self.danmakuComments = danmakuComments
        self.danmakuConfig = danmakuConfig
        self.showDanmaku = showDanmaku
    }

    public func makeNSView(context: Context) -> AVPlayerView {
        let view = AVPlayerView()
        view.controlsStyle = .inline
        view.showsFullScreenToggleButton = true
        view.player = player.avPlayer
        return view
    }

    public func updateNSView(_ nsView: AVPlayerView, context: Context) {
        // BuiltinPlayer 复用稳定的 AVPlayer 实例，不在 SwiftUI 更新事务中反复解绑。
        if nsView.player !== player.avPlayer {
            nsView.player = player.avPlayer
        }
    }
}

// MARK: - 含弹幕叠加层的包装视图

/// 内置播放器含弹幕叠加层：视频 + 弹幕 Canvas。
///
/// 弹幕由 `DanmakuClient` 驱动，通过 `danmakuComments` 绑定到视图；
/// `danmakuConfig` 控制弹幕样式（字号/透明度/速度/区域等）。
public struct BuiltinPlayerWithDanmaku: View {
    @ObservedObject var player: BuiltinPlayer
    let danmakuComments: [DanmakuComment]
    @Binding var danmakuConfig: DanmakuConfig
    let danmakuAvailable: Bool
    @State private var showDanmaku = true
    @State private var showDanmakuSettings = false

    public init(player: BuiltinPlayer,
                danmakuComments: [DanmakuComment],
                danmakuConfig: Binding<DanmakuConfig>,
                danmakuAvailable: Bool = true) {
        self.player = player
        self.danmakuComments = danmakuComments
        self._danmakuConfig = danmakuConfig
        self.danmakuAvailable = danmakuAvailable
    }

    public var body: some View {
        GeometryReader { geo in
            ZStack {
                // 视频层。
                if player.engine == .webFLV, let request = player.webFLVRequest {
                    WebFLVPlayerView(player: player, request: request)
                } else {
                    BuiltinPlayerView(player: player,
                                      danmakuComments: danmakuComments,
                                      danmakuConfig: danmakuConfig,
                                      showDanmaku: showDanmaku)
                }

                // 弹幕叠加层。
                if showDanmaku && !danmakuComments.isEmpty {
                    DanmakuOverlay(
                        comments: danmakuComments,
                        config: danmakuConfig,
                        size: geo.size,
                        isPlaying: player.isPlaying
                    )
                        .allowsHitTesting(false)  // 弹幕不拦截点击事件。
                }

                if let error = player.errorMessage {
                    VStack(spacing: 10) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.system(size: 30))
                        Text("无法播放当前线路")
                            .font(.headline)
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 420)
                    }
                    .padding(24)
                    .liquidGlassBackground(in: .rect(cornerRadius: 16))
                }

                // 弹幕控制栏（底部）。
                if danmakuAvailable {
                    VStack {
                        Spacer()
                        GlassEffectContainer(spacing: 8) {
                            HStack(spacing: 12) {
                                Button {
                                    showDanmaku.toggle()
                                } label: {
                                    Image(systemName: showDanmaku ? "text.bubble.fill" : "text.bubble")
                                        .font(.system(size: 16))
                                        .frame(width: 28, height: 24)
                                }
                                .buttonStyle(.plain)
                                .glassEffect(in: .capsule)
                                .help(showDanmaku ? "关闭弹幕" : "开启弹幕")

                                Button {
                                    showDanmakuSettings.toggle()
                                } label: {
                                    Image(systemName: "gearshape.fill")
                                        .font(.system(size: 14))
                                        .frame(width: 28, height: 24)
                                }
                                .buttonStyle(.plain)
                                .glassEffect(in: .capsule)
                                .help("弹幕设置")

                                Spacer()
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                        }
                        .padding(12)
                    }
                }

                // 弹幕设置面板（浮层）。
                if danmakuAvailable && showDanmakuSettings {
                    DanmakuSettingsPanel(config: $danmakuConfig, onClose: {
                        showDanmakuSettings = false
                    })
                    .transition(.scale.combined(with: .opacity))
                }
            }
        }
    }
}
