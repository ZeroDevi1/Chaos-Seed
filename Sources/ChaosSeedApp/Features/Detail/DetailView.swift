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

/// 直播间详情页：清晰度/线路列表 + IINA 外部播放器 + 本应用弹幕显示。
///
/// 复刻原型 `detail.jsx`，升级后支持：
/// - IINA 外部播放器（`IINALauncher`），通过系统调用拉起；
/// - 本应用保留播放历史与实时弹幕列表。
public struct DetailView: View {
    @EnvironmentObject private var appState: AppState
    @Environment(\.modelContext) private var modelContext
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
            if vm.externalPlaybackActive {
                externalPlaybackContent
            } else {
                qualityListContent
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .background(Color(nsColor: .windowBackgroundColor).ignoresSafeArea())
        .onAppear {
            vm.decode()
        }
        .onDisappear {
            vm.stopPlaybackSession()
        }
    }

    // MARK: - 工具栏

    private var toolbar: some View {
        HStack(spacing: 12) {
            Button(action: {
                vm.stopPlaybackSession()
                onBack()
            }) {
                Label("返回首页", systemImage: "chevron.left")
            }
            Spacer()
            Text("直播间详情")
                .font(.system(size: 16, weight: .bold))
                .lineLimit(1)
            Spacer()
            Spacer().frame(width: 100) // 平衡标题居中
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .liquidGlassBar()
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

    // MARK: - IINA 播放后的弹幕内容区

    private var externalPlaybackContent: some View {
        HStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    header
                    externalPlaybackStatus
                    qualitySection
                    if debugLogging {
                        logBox
                    }
                }
                .padding(20)
            }
            Divider()
            externalDanmakuRail
                .frame(width: 340)
        }
    }

    private var externalPlaybackStatus: some View {
        HStack(spacing: 12) {
            Image(systemName: "arrow.up.forward.app.fill")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(Color.accentColor)
            VStack(alignment: .leading, spacing: 3) {
                Text("IINA 播放中")
                    .font(.system(size: 14, weight: .semibold))
                Text("\(selectedQualityLabel) · \(danmakuStatusText)")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Button {
                vm.play(onResult: handlePlaybackResult)
            } label: {
                Label("重新打开 IINA", systemImage: "play.fill")
            }
            .buttonStyle(.bordered)
            .disabled(vm.selectedVariantId == nil || vm.loading)
        }
        .padding(14)
        .background(Color.gray.opacity(0.08), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    @ViewBuilder
    private var externalDanmakuRail: some View {
        if let client = vm.danmakuClient {
            ObservedDanmakuRail(client: client, config: vm.danmakuConfig)
        } else {
            DanmakuRightRail(comments: [], config: vm.danmakuConfig)
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
                    Button {
                        vm.play(onResult: handlePlaybackResult)
                    } label: {
                        Label("在 IINA 中播放", systemImage: "play.fill")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(vm.selectedVariantId == nil || vm.loading)

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
                            Text(qualitySubtitle(for: variant))
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(variantStatus(variant))
                            .font(.system(size: 11, weight: .semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(
                                variant.isResolved
                                ? Color.green.opacity(0.16)
                                : Color.gray.opacity(0.16),
                                in: Capsule()
                            )
                            .foregroundStyle(
                                variant.isResolved
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

    private func qualitySubtitle(for variant: StreamVariant) -> String {
        if let diagnostic = variant.biliQualityDiagnosticText {
            return diagnostic
        }
        return "线路 \((variant.quality % 3) + 1)"
    }

    private func variantStatus(_ variant: StreamVariant) -> String {
        if variant.isResolved {
            return "IINA 可播"
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
        case .iina:
            recordHistory(method: .iina)
            appState.showToast("已通过 IINA 打开「\(vm.room.title)」")
        case .iinaNotFound:
            appState.showToast("未检测到 IINA，请检查设置中的应用路径")
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

private struct ObservedDanmakuRail: View {
    @ObservedObject var client: DanmakuClient
    let config: DanmakuConfig

    var body: some View {
        DanmakuRightRail(
            comments: client.comments,
            config: config
        )
    }
}
