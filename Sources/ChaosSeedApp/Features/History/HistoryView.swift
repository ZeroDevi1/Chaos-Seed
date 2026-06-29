import SwiftUI
import SwiftData

/// 历史记录页。
public struct HistoryView: View {
    @Environment(\.modelContext) private var modelContext
    @Query(sort: \PlaybackHistoryEntry.playedAt, order: .reverse)
    private var entries: [PlaybackHistoryEntry]
    @State private var showClearConfirmation = false

    private let onOpenRoom: (LiveRoomCard) -> Void

    public init(onOpenRoom: @escaping (LiveRoomCard) -> Void) {
        self.onOpenRoom = onOpenRoom
    }

    public var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            if entries.isEmpty {
                EmptyStateView(
                    title: "暂无历史记录",
                    description: "开始播放后，直播间会出现在这里，方便快速回访。",
                    systemImage: "clock"
                )
            } else {
                List {
                    ForEach(entries) { entry in
                        historyRow(entry)
                            .contextMenu {
                                Button("重新打开") {
                                    onOpenRoom(entry.room)
                                }
                                Divider()
                                Button("删除记录", role: .destructive) {
                                    delete(entry)
                                }
                            }
                    }
                }
                .listStyle(.inset)
            }
        }
        .confirmationDialog(
            "清空全部历史记录？",
            isPresented: $showClearConfirmation
        ) {
            Button("清空历史", role: .destructive, action: clearHistory)
            Button("取消", role: .cancel) {}
        } message: {
            Text("此操作无法撤销。")
        }
    }

    private var toolbar: some View {
        HStack {
            Text("历史")
                .font(.system(size: 16, weight: .bold))
            Spacer()
            if !entries.isEmpty {
                Text("\(entries.count) 条")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                Button("清空历史", role: .destructive) {
                    showClearConfirmation = true
                }
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .liquidGlassBar()
    }

    private func historyRow(_ entry: PlaybackHistoryEntry) -> some View {
        HStack(spacing: 14) {
            CoverImage(cover: entry.cover, site: entry.site, placeholderIcon: "play.tv.fill")
                .frame(width: 128, height: 72)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            VStack(alignment: .leading, spacing: 7) {
                Text(entry.title)
                    .font(.system(size: 14, weight: .semibold))
                    .lineLimit(1)
                HStack(spacing: 8) {
                    PlatformBadge(site: entry.site)
                    Text(entry.userName ?? "未知主播")
                        .lineLimit(1)
                    if let quality = entry.qualityLabel {
                        Text(quality)
                    }
                    Text(entry.playbackMethod.label)
                }
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                Text(entry.playedAt, format: .relative(presentation: .named))
                    .font(.system(size: 11))
                    .foregroundStyle(.tertiary)
            }

            Spacer()

            Button {
                delete(entry)
            } label: {
                Image(systemName: "trash")
                    .frame(width: 30, height: 30)
            }
            .buttonStyle(.plain)
            .help("删除记录")
        }
        .padding(.vertical, 5)
        .contentShape(Rectangle())
        .onTapGesture {
            onOpenRoom(entry.room)
        }
    }

    private func delete(_ entry: PlaybackHistoryEntry) {
        do {
            try PlaybackHistoryStore.delete(entry, in: modelContext)
        } catch {
            Log.app.error("删除历史记录失败", error: error)
        }
    }

    private func clearHistory() {
        do {
            try PlaybackHistoryStore.deleteAll(in: modelContext)
        } catch {
            Log.app.error("清空历史记录失败", error: error)
        }
    }
}
