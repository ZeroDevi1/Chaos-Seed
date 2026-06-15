import SwiftUI

// MARK: - 房间卡片（对齐原型 RoomCard）

public struct RoomCard: View {
    let room: LiveRoomCard
    let onClick: () -> Void

    public init(room: LiveRoomCard, onClick: @escaping () -> Void) {
        self.room = room
        self.onClick = onClick
    }

    public var body: some View {
        Button(action: onClick) {
            VStack(alignment: .leading, spacing: 0) {
                ZStack(alignment: .bottomLeading) {
                    CoverImage(cover: room.cover, site: room.site, placeholderIcon: "play.tv.fill")
                        .aspectRatio(16.0/9.0, contentMode: .fit)
                    if let online = room.online {
                        HStack(spacing: 4) {
                            Image(systemName: "eye")
                                .font(.system(size: 10, weight: .bold))
                            Text(formatViewers(online))
                                .font(.system(size: 11, weight: .semibold))
                        }
                        .foregroundStyle(.white)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2)
                        .background(Color.black.opacity(0.65), in: Capsule())
                        .padding(8)
                    }
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text(room.title)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    HStack(spacing: 6) {
                        PlatformBadge(site: room.site)
                        Text(room.userName ?? "未知主播")
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }
                .padding(12)
            }
            .background(
                Color(NSColor.controlBackgroundColor),
                in: RoundedRectangle(cornerRadius: 10, style: .continuous)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(.separator)
            )
            .contentShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            // macOS 26 卡片使用 Liquid Glass 背景（带圆角）。
            .liquidGlassBackground(in: .rect(cornerRadius: 10))
        }
        .buttonStyle(.plain)
    }
}

// MARK: - 平台徽章

public struct PlatformBadge: View {
    let site: Site
    public init(site: Site) { self.site = site }

    public var body: some View {
        Text(site.displayName)
            .font(.system(size: 10, weight: .bold))
            .textCase(.uppercase)
            .foregroundStyle(.white)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Theme.badgeColor(for: site), in: RoundedRectangle(cornerRadius: 4, style: .continuous))
    }
}

// MARK: - 分类栏（对齐原型 CategoryBar，两级）

public struct CategoryBar: View {
    let categories: [LiveCategory]
    let selectedCategory: String
    let selectedSub: String
    let onSelectCategory: (String) -> Void
    let onSelectSub: (String) -> Void

    public init(categories: [LiveCategory],
                selectedCategory: String,
                selectedSub: String,
                onSelectCategory: @escaping (String) -> Void,
                onSelectSub: @escaping (String) -> Void) {
        self.categories = categories
        self.selectedCategory = selectedCategory
        self.selectedSub = selectedSub
        self.onSelectCategory = onSelectCategory
        self.onSelectSub = onSelectSub
    }

    public var body: some View {
        let children = categories.first(where: { $0.id == selectedCategory })?.children ?? []
        VStack(alignment: .leading, spacing: 8) {
            chipRow(items: categories.map { CategoryChip(id: $0.id, name: $0.name) },
                    selected: selectedCategory) { newCat in
                onSelectCategory(newCat)
            }
            if !children.isEmpty {
                chipRow(items: children.map { CategoryChip(id: $0.id, name: $0.name) },
                        selected: selectedSub) { onSelectSub($0) }
            }
        }
    }

    private func chipRow(items: [CategoryChip], selected: String, onSelect: @escaping (String) -> Void) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(items) { chip in
                    let isSelected = chip.id == selected
                    Button(action: { onSelect(chip.id) }) {
                        Text(chip.name)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(isSelected ? .white : .secondary)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 4)
                            .background(
                                isSelected ? Color.accentColor : Color.gray.opacity(0.14),
                                in: Capsule()
                            )
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private struct CategoryChip: Identifiable, Hashable { let id: String; let name: String }
}

// MARK: - 搜索框（对齐原型 SearchBox）

public struct SearchBox: View {
    @Binding var text: String
    let onSubmit: () -> Void

    public init(text: Binding<String>, onSubmit: @escaping () -> Void) {
        self._text = text
        self.onSubmit = onSubmit
    }

    public var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
            TextField("搜索主播或标题…", text: $text)
                .textFieldStyle(.plain)
                .onSubmit(onSubmit)
            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.tertiary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 30)
        .frame(maxWidth: 320)
        .background(Color.gray.opacity(0.14), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

// MARK: - 分页栏（对齐原型 .pagination）

public struct PaginationBar: View {
    let page: Int
    let totalPages: Int
    let onPrev: () -> Void
    let onNext: () -> Void

    public init(page: Int, totalPages: Int, onPrev: @escaping () -> Void, onNext: @escaping () -> Void) {
        self.page = page
        self.totalPages = totalPages
        self.onPrev = onPrev
        self.onNext = onNext
    }

    public var body: some View {
        HStack(spacing: 12) {
            Button("上一页", action: onPrev)
                .buttonStyle(.bordered)
                .disabled(page <= 1)
            Text("\(page) / \(max(1, totalPages))")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .frame(minWidth: 70)
            Button("下一页", action: onNext)
                .buttonStyle(.bordered)
                .disabled(page >= totalPages)
        }
        .padding(.vertical, 12)
    }
}

// MARK: - 空状态（对齐原型 EmptyState）

public struct EmptyStateView: View {
    let title: String
    let description: String
    let systemImage: String

    public init(title: String, description: String, systemImage: String = "magnifyingglass") {
        self.title = title
        self.description = description
        self.systemImage = systemImage
    }

    public var body: some View {
        VStack(spacing: 12) {
            Image(systemName: systemImage)
                .font(.system(size: 36, weight: .regular))
                .foregroundStyle(.secondary.opacity(0.6))
            Text(title)
                .font(.system(size: 16, weight: .semibold))
            Text(description)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - 加载指示器（对齐原型 loading-spinner）

public struct LoadingSpinner: View {
    public init() {}
    public var body: some View {
        ProgressView()
            .controlSize(.large)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - URL 解析弹窗（对齐原型 Modal）

public struct URLParseModal: View {
    @Binding var isPresented: Bool
    @Binding var input: String
    let isParsing: Bool
    let onConfirm: () -> Void

    public init(isPresented: Binding<Bool>,
                input: Binding<String>,
                isParsing: Bool,
                onConfirm: @escaping () -> Void) {
        self._isPresented = isPresented
        self._input = input
        self.isParsing = isParsing
        self.onConfirm = onConfirm
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("解析直播间 URL")
                .font(.system(size: 15, weight: .bold))
            Text("支持 BiliLive、Douyu、Huya 的直播间链接或平台前缀。")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
            TextField("https://live.bilibili.com/12345", text: $input)
                .textFieldStyle(.roundedBorder)
                .onSubmit(onConfirm)
            HStack {
                Spacer()
                Button("取消") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button(isParsing ? "解析中…" : "解析", action: onConfirm)
                    .buttonStyle(.borderedProminent)
                    .disabled(input.trimmingCharacters(in: .whitespaces).isEmpty || isParsing)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 420)
    }
}
