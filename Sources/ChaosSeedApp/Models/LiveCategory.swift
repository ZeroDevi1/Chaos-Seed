import Foundation

/// 直播二级分类。
///
/// 对齐 Rust `chaos_core::live_directory::model::LiveSubCategory`。
public struct LiveSubCategory: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var parentId: String
    public var name: String
    public var pic: String?

    public init(id: String, parentId: String, name: String, pic: String? = nil) {
        self.id = id
        self.parentId = parentId
        self.name = name
        self.pic = pic
    }
}

/// 直播一级分类（含二级子分类）。
///
/// 对齐 Rust `chaos_core::live_directory::model::LiveCategory`。
public struct LiveCategory: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var children: [LiveSubCategory]

    public init(id: String, name: String, children: [LiveSubCategory] = []) {
        self.id = id
        self.name = name
        self.children = children
    }
}
