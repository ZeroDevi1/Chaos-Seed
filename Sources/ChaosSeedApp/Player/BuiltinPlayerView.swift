import SwiftUI
import AVFoundation
import AppKit

/// 内置播放器的视频渲染视图。
///
/// 特性：
/// - 使用 `AVPlayerLayer` 绕过 macOS 27 Beta 中 `AVPlayerView` 原生控件的绑定崩溃；
/// - 自动绑定 `BuiltinPlayer.avPlayer` 到视频渲染层；
/// - HDR（EDR）由视频解码管线自动启用。
public struct BuiltinPlayerView: NSViewRepresentable {
    let player: BuiltinPlayer

    public init(player: BuiltinPlayer) {
        self.player = player
    }

    public func makeNSView(context: Context) -> PlayerLayerView {
        let view = PlayerLayerView()
        view.playerLayer.player = player.avPlayer
        player.attachPictureInPicture(to: view.playerLayer)
        return view
    }

    public func updateNSView(_ nsView: PlayerLayerView, context: Context) {
        if nsView.playerLayer.player !== player.avPlayer {
            nsView.playerLayer.player = player.avPlayer
        }
        player.attachPictureInPicture(to: nsView.playerLayer)
    }

    public static func dismantleNSView(_ nsView: PlayerLayerView, coordinator: ()) {
        nsView.playerLayer.player = nil
    }
}

/// 仅负责视频呈现，不创建 AVKit 的 SwiftUI 控件树。
public final class PlayerLayerView: NSView {
    var playerLayer: AVPlayerLayer {
        guard let playerLayer = layer as? AVPlayerLayer else {
            preconditionFailure("PlayerLayerView requires AVPlayerLayer")
        }
        return playerLayer
    }

    public override func makeBackingLayer() -> CALayer {
        let layer = AVPlayerLayer()
        layer.videoGravity = .resizeAspect
        layer.backgroundColor = NSColor.black.cgColor
        return layer
    }

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
    }
}

// MARK: - 含弹幕叠加层的包装视图

enum DanmakuPresentationPolicy {
    static func showsOverlay(isEnabled: Bool) -> Bool {
        isEnabled
    }

    static func showsRightRail(isExpanded: Bool) -> Bool {
        isExpanded
    }

    static func rightRailWidth(totalWidth: CGFloat, isExpanded: Bool) -> CGFloat {
        guard isExpanded else { return 0 }
        return min(340, max(280, totalWidth * 0.28))
    }

    static func videoWidth(totalWidth: CGFloat, isRightRailExpanded: Bool) -> CGFloat {
        max(
            0,
            totalWidth - rightRailWidth(
                totalWidth: totalWidth,
                isExpanded: isRightRailExpanded
            )
        )
    }
}

/// 内置播放器含弹幕叠加层：视频 + 弹幕 Canvas。
///
/// 弹幕由 `DanmakuClient` 驱动，通过 `danmakuComments` 绑定到视图；
/// `danmakuConfig` 控制弹幕样式（字号/透明度/速度/区域等）。
public struct BuiltinPlayerWithDanmaku: View {
    @ObservedObject var player: BuiltinPlayer
    let danmakuComments: [DanmakuComment]
    let danmakuConfig: DanmakuConfig
    let showOverlayDanmaku: Bool
    @Binding var isRightRailExpanded: Bool

    public init(player: BuiltinPlayer,
                danmakuComments: [DanmakuComment],
                danmakuConfig: DanmakuConfig,
                showOverlayDanmaku: Bool = true,
                isRightRailExpanded: Binding<Bool>) {
        self.player = player
        self.danmakuComments = danmakuComments
        self.danmakuConfig = danmakuConfig
        self.showOverlayDanmaku = showOverlayDanmaku
        self._isRightRailExpanded = isRightRailExpanded
    }

    public var body: some View {
        GeometryReader { geo in
            let railWidth = DanmakuPresentationPolicy.rightRailWidth(
                totalWidth: geo.size.width,
                isExpanded: isRightRailExpanded
            )
            let videoWidth = DanmakuPresentationPolicy.videoWidth(
                totalWidth: geo.size.width,
                isRightRailExpanded: isRightRailExpanded
            )

            HStack(spacing: 0) {
                ZStack(alignment: .trailing) {
                    videoSurface(size: CGSize(width: videoWidth, height: geo.size.height))

                    Button {
                        isRightRailExpanded.toggle()
                    } label: {
                        Image(systemName: isRightRailExpanded ? "chevron.right" : "chevron.left")
                            .font(.system(size: 10, weight: .bold))
                            .frame(width: 24, height: 42)
                            .foregroundStyle(.white)
                            .background(.black.opacity(0.58), in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .padding(.trailing, 10)
                    .help(isRightRailExpanded ? "收起弹幕列表" : "展开弹幕列表")
                }
                .frame(width: videoWidth)

                if DanmakuPresentationPolicy.showsRightRail(isExpanded: isRightRailExpanded) {
                    DanmakuRightRail(
                        comments: danmakuComments,
                        config: danmakuConfig
                    )
                    .frame(width: railWidth)
                    .background(.ultraThinMaterial)
                    .overlay(alignment: .leading) {
                        Rectangle()
                            .fill(Color.white.opacity(0.12))
                            .frame(width: 1)
                    }
                    .transition(.move(edge: .trailing).combined(with: .opacity))
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .clipped()
            .animation(.easeInOut(duration: 0.2), value: isRightRailExpanded)
        }
    }

    private func videoSurface(size: CGSize) -> some View {
        ZStack {
            if player.engine == .libMPV {
                LibMPVPlayerView(player: player)
            } else if player.engine == .webFLV, let request = player.webFLVRequest {
                WebFLVPlayerView(player: player, request: request)
            } else {
                BuiltinPlayerView(player: player)
            }

            if DanmakuPresentationPolicy.showsOverlay(isEnabled: showOverlayDanmaku),
               !danmakuComments.isEmpty {
                DanmakuOverlay(
                    comments: danmakuComments,
                    config: danmakuConfig,
                    size: size,
                    isPlaying: player.isPlaying
                )
                .allowsHitTesting(false)
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
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.black)
        .clipped()
    }
}
