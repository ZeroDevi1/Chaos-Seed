import Foundation

/// 首页浏览位置书签：持久化平台、一级分类、二级分类与页码，确保退出/返回后状态不丢失。
///
/// 存入 `UserDefaults`（通过 `Codable` + JSON），启动时恢复。
/// 是否启用记忆由独立的 `@AppStorage(\"rememberBrowsePosition\")` 控制（见 SettingsView）。
public struct HomeBookmark: Codable, Equatable, Sendable {
    /// 上次选中的平台，默认 BiliLive。
    public var platform: Site
    /// 上次选中的一级分类 id。
    public var category: String
    /// 上次选中的二级分类 id（空字符串表示一级分类下的全部）。
    public var subCategory: String
    /// 上次浏览的页码。
    public var page: Int

    public init(
        platform: Site = .biliLive,
        category: String = "recommend",
        subCategory: String = "",
        page: Int = 1
    ) {
        self.platform = platform
        self.category = category
        self.subCategory = subCategory
        self.page = page
    }

    // MARK: - UserDefaults 读写

    private static let defaultsKey = "HomeBookmark"

    /// 从 UserDefaults 加载书签，无数据时返回默认值。
    public static func load() -> HomeBookmark {
        guard let data = UserDefaults.standard.data(forKey: defaultsKey),
              let bookmark = try? JSONDecoder().decode(HomeBookmark.self, from: data)
        else {
            return HomeBookmark()
        }
        return bookmark
    }

    /// 将当前书签写入 UserDefaults。
    public func save() {
        guard let data = try? JSONEncoder().encode(self) else { return }
        UserDefaults.standard.set(data, forKey: Self.defaultsKey)
    }

    /// 清空持久化书签（恢复默认值）。
    public static func clear() {
        UserDefaults.standard.removeObject(forKey: defaultsKey)
    }
}
