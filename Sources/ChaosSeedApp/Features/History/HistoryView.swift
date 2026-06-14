import SwiftUI

/// 历史记录页。
///
/// v1：仅空状态占位（对齐原型）。SwiftData 持久化留待后续迭代。
public struct HistoryView: View {
    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            EmptyStateView(
                title: "暂无历史记录",
                description: "播放过的直播间会出现在这里，方便快速回访。",
                systemImage: "clock"
            )
        }
    }

    private var toolbar: some View {
        HStack {
            Text("历史")
                .font(.system(size: 16, weight: .bold))
            Spacer()
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .background(.bar)
    }
}
