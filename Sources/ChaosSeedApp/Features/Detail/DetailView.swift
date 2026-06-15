import SwiftUI

/// 直播间详情页：清晰度/线路列表 + 内置播放器 / IINA 外部播放器。
///
/// 复刻原型 `detail.jsx`，升级后支持：
/// - 内置 AVPlayer 播放器（`BuiltinPlayer`），直接在应用内渲染视频；
/// - IINA 外部播放器（`IINALauncher`），通过系统调用拉起；
/// - 设置页可切换默认播放器偏好。
public struct DetailView: View {
    @EnvironmentObject private var appState: AppState
    @AppStorage("debugLogging") private var debugLogging = false
    @StateObject private var vm: DetailViewModel
    let onBack: () -> Void

    public init(liveKit: LiveKit, room: LiveRoomCard, onBack: @escaping () -> Void) {
        self._vm = StateObject(wrappedValue: DetailViewModel(room: room, liveKit: liveKit))
        self.onBack = onBack
    }

    public var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            // 当内置播放器激活时，用 AVPlayerView 替换整个内容区。
            if let player = vm.builtinPlayer {
                builtinPlayerContent(player: player)
            } else {
                qualityListContent
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .background(Color(nsColor: .windowBackgroundColor).ignoresSafeArea())
        .onAppear { vm.decode() }
        .onDisappear { vm.stopBuiltinPlayer() }
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
        VStack(spacing: 0) {
            // 视频渲染层 + 弹幕叠加。
            playerSurface(player: player)
            .aspectRatio(16.0 / 9.0, contentMode: .fit)
            .background(Color.black)
            .onAppear {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    BuiltinPlayer.configureWindowForHDR(NSApp.keyWindow)
                }
            }

            Divider()

            // 播放中清晰度切换栏（紧凑版）。
            GlassEffectContainer(spacing: 8) {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(vm.variants) { variant in
                            let isSelected = vm.selectedVariantId == variant.id
                            Button {
                                vm.selectedVariantId = variant.id
                                vm.play(onResult: handlePlaybackResult)
                            } label: {
                                Text(variant.label)
                                    .font(.system(size: 12, weight: .medium))
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 4)
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(isSelected ? Color.white : Color.primary)
                            .background(isSelected ? Color.accentColor : Color.clear, in: Capsule())
                            .glassEffect(in: .capsule)
                        }
                    }
                    .padding(.horizontal, 16)
                }
            }
            .frame(height: 44)
        }
    }

    @ViewBuilder
    private func playerSurface(player: BuiltinPlayer) -> some View {
        if let danmakuClient = vm.danmakuClient {
            ObservedDanmakuPlayer(
                player: player,
                client: danmakuClient,
                config: $vm.danmakuConfig
            )
        } else {
            BuiltinPlayerWithDanmaku(
                player: player,
                danmakuComments: [],
                danmakuConfig: $vm.danmakuConfig,
                danmakuAvailable: vm.room.site == .biliLive
            )
        }
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
            return source.engine == .avFoundation ? "内置 HLS" : "内置 FLV"
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
            appState.showToast("已通过 IINA 打开「\(vm.room.title)」")
        case .iinaNotFound:
            appState.showToast("未检测到 IINA，请检查设置中的应用路径")
        case .unsupportedBuiltin:
            appState.showToast("当前线路仅提供不受支持的 P2P 地址，请切换 IINA")
        case .failure(let message):
            appState.showToast(message)
        }
    }
}

private struct ObservedDanmakuPlayer: View {
    @ObservedObject var player: BuiltinPlayer
    @ObservedObject var client: DanmakuClient
    @Binding var config: DanmakuConfig

    var body: some View {
        BuiltinPlayerWithDanmaku(
            player: player,
            danmakuComments: client.comments,
            danmakuConfig: $config
        )
    }
}
