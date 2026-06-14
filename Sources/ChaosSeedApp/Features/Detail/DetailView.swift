import SwiftUI

/// 直播间详情页：展示清晰度/线路列表并调用 IINA 播放。
///
/// 复刻原型 `detail.jsx`。
public struct DetailView: View {
    @EnvironmentObject private var appState: AppState
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
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    header
                    qualitySection
                    logBox
                }
                .padding(20)
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .onAppear { vm.decode() }
    }

    private var toolbar: some View {
        HStack(spacing: 12) {
            Button(action: onBack) {
                Label("返回首页", systemImage: "chevron.left")
            }
            Spacer()
            Text("直播间详情")
                .font(.system(size: 16, weight: .bold))
            Spacer()
            Spacer().frame(width: 100) // 平衡标题居中
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .background(.bar)
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 20) {
            Rectangle()
                .fill(
                    LinearGradient(
                        colors: [.gray.opacity(0.25), .gray.opacity(0.45)],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: 280, height: 158)
                .overlay(
                    Text("封面占位")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(.primary.opacity(0.3))
                )
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
                        vm.play { result in
                            if case .launched = result {
                                appState.showToast("已通过 IINA 打开「\(vm.room.title)」")
                            } else if case .iinaNotFound = result {
                                appState.showToast("未检测到 IINA，请安装后重试")
                            }
                        }
                    } label: {
                        Label("在 IINA 中播放", systemImage: "play.fill")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(vm.selectedVariantId == nil || vm.loading)

                    Button {
                        if let url = vm.copySelectedUrl() ?? vm.variants.first(where: { $0.id == vm.selectedVariantId })?.url {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(url, forType: .string)
                            appState.showToast("已复制直连 URL 到剪贴板（原型模拟）")
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
                        Text(variant.isResolved ? "已解析" : "需二段解析")
                            .font(.system(size: 11, weight: .semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(
                                variant.isResolved
                                ? Color.green.opacity(0.16)
                                : Color.gray.opacity(0.16),
                                in: Capsule()
                            )
                            .foregroundStyle(variant.isResolved ? Color(red: 0.12, green: 0.54, blue: 0.18) : .secondary)
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
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(.separator)
        )
    }

    private func lineName(for variant: StreamVariant) -> String {
        // 简单：用 quality 数字映射到"线路N"，便于复刻原型观感。
        "线路 \((variant.quality % 3) + 1)"
    }

    private var logBox: some View {
        Text(vm.logs.isEmpty ? "（无日志）" : vm.logs)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
            .padding(12)
            .background(Color.gray.opacity(0.08), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}
