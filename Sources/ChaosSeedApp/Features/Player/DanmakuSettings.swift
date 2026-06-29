import SwiftUI

/// 视频画面内悬浮弹幕的配置面板。
///
/// 对标 BiliBili 播放器弹幕设置：
/// - 字号、不透明度、速度、显示区域、显示模式
/// - 彩色弹幕开关
/// - 屏蔽词管理（关键词过滤）
public struct DanmakuSettingsPanel: View {
    @Binding var config: DanmakuConfig
    /// 关闭面板回调。
    let onClose: () -> Void

    @State private var newBlockedWord: String = ""

    public init(config: Binding<DanmakuConfig>, onClose: @escaping () -> Void) {
        self._config = config
        self.onClose = onClose
    }

    public var body: some View {
        VStack(spacing: 0) {
            // 标题栏。
            HStack {
                Text("弹幕设置")
                    .font(.system(size: 15, weight: .bold))
                Spacer()
                Button(action: onClose) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.title3)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("这里的设置只影响视频画面内从右向左移动的悬浮弹幕，右侧消息栏始终独立显示。")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                    fontSizeRow
                    opacityRow
                    minOpacityRow
                    speedRow
                    displayAreaRow
                    typeFilters
                    colorToggleRow
                    duplicateToggleRow
                    blockedWordsSection
                }
                .padding(16)
            }
        }
        .frame(width: 320, height: 500)
        .liquidGlassBackground(in: .rect(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.separator))
        .shadow(color: .black.opacity(0.2), radius: 16, y: 8)
    }

    // MARK: - 控制行

    private var fontSizeRow: some View {
        labeledRow("字号") {
            Slider(value: $config.fontSize, in: 12...48, step: 2) {
                Text("字号")
            }
            Text("\(Int(config.fontSize)) pt")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .frame(width: 40, alignment: .trailing)
        }
    }

    private var opacityRow: some View {
        labeledRow("不透明度") {
            Slider(value: $config.opacity, in: 0.1...1.0, step: 0.05) {
                Text("不透明度")
            }
            Text(String(format: "%.0f%%", config.opacity * 100))
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .frame(width: 40, alignment: .trailing)
        }
    }

    private var speedRow: some View {
        labeledRow("滚动速度") {
            Slider(value: $config.speed, in: 0.5...3.0, step: 0.1) {
                Text("速度")
            }
            Text(String(format: "%.1fx", config.speed))
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .frame(width: 40, alignment: .trailing)
        }
    }

    private var minOpacityRow: some View {
        labeledRow("透明弹幕过滤") {
            Slider(value: $config.minOpacity, in: 0...1, step: 0.05) {
                Text("最低透明度")
            }
            Text(String(format: "≥ %.0f%%", config.minOpacity * 100))
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .frame(width: 48, alignment: .trailing)
        }
    }

    private var displayAreaRow: some View {
        labeledRow("显示区域") {
            Picker("", selection: $config.displayAreaRatio) {
                Text("1/4 屏").tag(0.25)
                Text("半屏").tag(0.5)
                Text("全屏").tag(1.0)
            }
            .pickerStyle(.segmented)
        }
    }

    private var typeFilters: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("弹幕类型")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
            HStack(spacing: 8) {
                danmakuTypeToggle("滚动", isOn: $config.showScrolling)
                danmakuTypeToggle("顶部", isOn: $config.showTop)
                danmakuTypeToggle("底部", isOn: $config.showBottom)
            }
        }
    }

    private func danmakuTypeToggle(_ title: String, isOn: Binding<Bool>) -> some View {
        Button {
            isOn.wrappedValue.toggle()
        } label: {
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .foregroundStyle(isOn.wrappedValue ? Color.white : Color.primary)
                .background(
                    isOn.wrappedValue ? Color.accentColor : Color.gray.opacity(0.12),
                    in: RoundedRectangle(cornerRadius: 6, style: .continuous)
                )
        }
        .buttonStyle(.plain)
    }

    private var colorToggleRow: some View {
        HStack {
            Text("彩色弹幕")
                .font(.system(size: 13))
            Spacer()
            Toggle("", isOn: $config.showColored)
                .toggleStyle(.switch)
                .labelsHidden()
        }
    }

    private var duplicateToggleRow: some View {
        HStack {
            Text("重复弹幕折叠")
                .font(.system(size: 13))
            Spacer()
            Toggle("", isOn: $config.collapseDuplicates)
                .toggleStyle(.switch)
                .labelsHidden()
        }
    }

    // MARK: - 屏蔽词

    private var blockedWordsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("屏蔽词管理")
                .font(.system(size: 13, weight: .semibold))

            // 添加屏蔽词。
            HStack(spacing: 6) {
                TextField("关键词或 /正则/", text: $newBlockedWord)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 12))
                Button("添加") {
                    let trimmed = newBlockedWord.trimmingCharacters(in: .whitespaces)
                    guard DanmakuLayoutEngine.isValidBlockRule(trimmed),
                          !config.blockedWords.contains(trimmed) else { return }
                    config.blockedWords.append(trimmed)
                    newBlockedWord = ""
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(
                    !DanmakuLayoutEngine.isValidBlockRule(
                        newBlockedWord.trimmingCharacters(in: .whitespacesAndNewlines)
                    )
                )
            }

            Text("使用 /表达式/ 添加不区分大小写的正则规则")
                .font(.system(size: 10))
                .foregroundStyle(.secondary)

            // 已屏蔽词列表。
            if config.blockedWords.isEmpty {
                Text("暂无屏蔽词")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 4) {
                        ForEach(config.blockedWords, id: \.self) { word in
                            HStack(spacing: 2) {
                                Text(word)
                                    .font(.system(size: 11))
                                Button {
                                    config.blockedWords.removeAll { $0 == word }
                                } label: {
                                    Image(systemName: "xmark")
                                        .font(.system(size: 8))
                                }
                                .buttonStyle(.plain)
                            }
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.gray.opacity(0.15), in: Capsule())
                        }
                    }
                }
            }
        }
    }

    // MARK: - 辅助

    @ViewBuilder
    private func labeledRow<C: View>(_ label: String, @ViewBuilder content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
            HStack(spacing: 8) { content() }
        }
    }
}
