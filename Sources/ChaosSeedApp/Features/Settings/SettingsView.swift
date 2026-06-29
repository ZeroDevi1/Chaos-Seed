import SwiftUI

/// 设置页：外观主题、IINA 路径、网络超时、调试日志、记忆浏览位置。
///
/// 复刻原型 `settings.jsx` 的控件；持久化用 `@AppStorage`。
public struct SettingsView: View {
    @AppStorage("iinaPath") private var iinaPath = "/Applications/IINA.app"
    @AppStorage("networkTimeout") private var networkTimeout = 30
    @AppStorage("debugLogging") private var debugLogging = false
    @AppStorage("useMockLiveKit") private var useMockLiveKit = false
    /// 是否在启动时恢复上次浏览位置（平台/分类/页码）。
    @AppStorage("rememberBrowsePosition") private var rememberBrowsePosition = false
    @StateObject private var biliAccountVM = BiliAccountSettingsViewModel()
    @State private var showingBiliLogin = false
    @Binding var appearance: AppAppearance

    public init(appearance: Binding<AppAppearance>) {
        self._appearance = appearance
    }

    public var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            ScrollView {
                VStack(spacing: 0) {
                    appearanceRow
                    Divider()
                    rememberRow
                    Divider()
                    biliAccountRow
                    Divider()
                    iinaPathRow
                    Divider()
                    timeoutRow
                    Divider()
                    debugRow
                    Divider()
                    mockRow
                }
                .frame(maxWidth: 560, alignment: .leading)
                .background(Color(NSColor.controlBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(.separator))
                .liquidGlassBackground(in: .rect(cornerRadius: 10))
                .padding(20)
            }
        }
        .sheet(isPresented: $showingBiliLogin) {
            BiliQRCodeLoginSheet {
                showingBiliLogin = false
                Task { await biliAccountVM.refresh() }
            }
        }
        .task {
            await biliAccountVM.refresh()
        }
    }

    private var toolbar: some View {
        HStack {
            Text("设置")
                .font(.system(size: 16, weight: .bold))
            Spacer()
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .liquidGlassBar()
    }

    private var appearanceRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("外观")
                    .font(.system(size: 13, weight: .semibold))
                Text("跟随系统、浅色或深色。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Picker("外观", selection: $appearance) {
                ForEach(AppAppearance.allCases, id: \.self) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 220)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    /// 记住浏览位置开关：开启后退出应用重开会恢复上次的平台/分类/页码。
    private var rememberRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("记住浏览位置")
                    .font(.system(size: 13, weight: .semibold))
                Text("关闭应用后重新打开时，恢复上次选择的平台、分类与页码。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Toggle("", isOn: $rememberBrowsePosition)
                .toggleStyle(.switch)
                .labelsHidden()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private var iinaPathRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("IINA 应用路径")
                    .font(.system(size: 13, weight: .semibold))
                Text("用于播放解析后的直播流。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            TextField("/Applications/IINA.app", text: $iinaPath)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 280)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private var biliAccountRow: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Bilibili 账号")
                    .font(.system(size: 13, weight: .semibold))
                Text(biliAccountVM.subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if biliAccountVM.loading {
                ProgressView()
                    .controlSize(.small)
            }
            if biliAccountVM.isLoggedIn {
                Button("退出登录") {
                    biliAccountVM.logout()
                }
                .buttonStyle(.bordered)
            } else {
                Button("扫码登录") {
                    showingBiliLogin = true
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private var timeoutRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("网络超时（秒）")
                    .font(.system(size: 13, weight: .semibold))
                Text("调用解析时的最大等待时间。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Stepper(value: $networkTimeout, in: 5...120) {
                Text("\(networkTimeout) 秒")
                    .frame(width: 80, alignment: .trailing)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private var debugRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("调试日志")
                    .font(.system(size: 13, weight: .semibold))
                Text("在详情页显示解析过程的详细日志。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Toggle("", isOn: $debugLogging)
                .toggleStyle(.switch)
                .labelsHidden()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private var mockRow: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("使用内置演示数据")
                    .font(.system(size: 13, weight: .semibold))
                Text("开启后用本地样本数据，不访问真实平台接口（重启应用生效）。")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Toggle("", isOn: $useMockLiveKit)
                .toggleStyle(.switch)
                .labelsHidden()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }
}
