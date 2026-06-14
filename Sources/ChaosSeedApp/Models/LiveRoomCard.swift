import Foundation

/// 直播间卡片（目录浏览返回的最小房间信息）。
///
/// 对齐 Rust `chaos_core::live_directory::model::LiveRoomCard`。
/// 字段名与 Rust 端保持一致，以便后续真实网络层零改动替换 Mock。
public struct LiveRoomCard: Codable, Hashable, Sendable, Identifiable {
    /// 用 `input` 作为 SwiftUI 的稳定标识（同平台同房间号唯一）。
    public var id: String { input }

    public var site: Site
    public var roomId: String
    /// `bilibili:<rid>` / `huya:<rid>` / `douyu:<rid>`，可直接喂给 `decodeManifest`。
    public var input: String
    public var title: String
    public var cover: String?
    public var userName: String?
    public var online: Int?

    public init(
        site: Site,
        roomId: String,
        input: String,
        title: String,
        cover: String? = nil,
        userName: String? = nil,
        online: Int? = nil
    ) {
        self.site = site
        self.roomId = roomId
        self.input = input
        self.title = title
        self.cover = cover
        self.userName = userName
        self.online = online
    }
}

/// 直播间分页列表。
///
/// 对齐 Rust `chaos_core::live_directory::model::LiveRoomList`。
/// 注意 Rust 端只有 `has_more`，没有 `total`——UI 的翻页上限由 `has_more` 决定。
public struct LiveRoomList: Codable, Hashable, Sendable {
    public var hasMore: Bool
    public var items: [LiveRoomCard]

    public init(hasMore: Bool, items: [LiveRoomCard]) {
        self.hasMore = hasMore
        self.items = items
    }
}
