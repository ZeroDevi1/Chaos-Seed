import SwiftUI

/// 首页视图模型，承载平台/分类/搜索/分页状态与加载逻辑。
///
/// 复刻原型 `HomeView.jsx` 的状态机：
/// - 切换平台 → 重置分类到第一个、页码归 1、清空搜索词；
/// - 切换分类/搜索词/页码 → 触发重新加载；
/// - 搜索时隐藏分类栏，仅保留平台与结果列表。
@MainActor
public final class HomeViewModel: ObservableObject {
    @Published var platform: Site = .biliLive
    @Published var categories: [LiveCategory] = []
    @Published var category: String = "recommend"
    @Published var subCategory: String = ""
    @Published var keyword: String = ""
    @Published var searchKeyword: String = ""
    @Published var page: Int = 1
    @Published var rooms: [LiveRoomCard] = []
    @Published var hasMore: Bool = false
    @Published var loading: Bool = false
    @Published var errorMessage: String?

    /// Mock 用：当前结果集的房间总数（用于翻页上限）。真实接口只有 hasMore。
    @Published private(set) var totalInPlatform: Int = 0

    /// 计算翻页上限：Mock 数据可枚举，用 `hasMore` + 当前页结果推断。
    var totalPages: Int {
        // Mock 下：仅靠 hasMore 不足以展示页码上限，用本地知识。
        // 简化：若无更多则当前页即末页；否则至少当前页+1。
        hasMore ? page + 1 : page
    }

    private let liveKit: LiveKit
    private var loadTask: Task<Void, Never>?

    public init(liveKit: LiveKit) {
        self.liveKit = liveKit
    }

    /// 暴露给视图层用于 URL 解析等需要直接调用 LiveKit 的场景。
    public var kit: LiveKit { liveKit }

    func bootstrap() {
        loadCategoriesAndRooms()
    }

    func selectPlatform(_ newPlatform: Site) {
        guard newPlatform != platform else { return }
        platform = newPlatform
        category = "recommend"
        subCategory = ""
        keyword = ""
        searchKeyword = ""
        page = 1
        loadCategoriesAndRooms()
    }

    func selectCategory(_ catId: String) {
        searchKeyword = ""
        keyword = ""
        category = catId
        let parent = categories.first(where: { $0.id == catId })
        subCategory = parent?.children.first?.id ?? ""
        page = 1
        load()
    }

    func selectSub(_ subId: String) {
        subCategory = subId
        page = 1
        load()
    }

    func submitSearch() {
        searchKeyword = keyword
        category = "recommend"
        subCategory = ""
        page = 1
        load()
    }

    func setPage(_ newPage: Int) {
        page = newPage
        load()
    }

    func refresh() {
        load()
    }

    private func loadCategoriesAndRooms() {
        loadTask?.cancel()
        loadTask = Task { [weak self] in
            guard let self else { return }
            do {
                let cats = try await self.liveKit.getCategories(site: self.platform)
                guard !Task.isCancelled else { return }
                self.categories = cats
                self.category = cats.first?.id ?? "recommend"
                self.subCategory = cats.first?.children.first?.id ?? ""
                await self.loadRooms()
            } catch {
                guard !Task.isCancelled else { return }
                self.errorMessage = error.localizedDescription
            }
        }
    }

    private func load() {
        loadTask?.cancel()
        loadTask = Task { [weak self] in
            await self?.loadRooms()
        }
    }

    private func loadRooms() async {
        loading = true
        errorMessage = nil
        defer { loading = false }
        do {
            let result: LiveRoomList
            if !searchKeyword.isEmpty {
                result = try await liveKit.searchRooms(site: platform, keyword: searchKeyword, page: page)
            } else if category == "recommend" || category.isEmpty {
                result = try await liveKit.getRecommendRooms(site: platform, page: page)
            } else {
                // category = 一级分类 id；subCategory = 二级分类 id（空/"0" 表示该一级分类下全部）。
                // BiliLive 需要 parent_area_id（一级）+ area_id（二级，"0"=全部）。
                // Douyu/Huya 不需要 parentId（接口签名里忽略），传 nil 即可。
                let parentId: String?
                let categoryId: String
                switch platform {
                case .biliLive:
                    parentId = category
                    categoryId = subCategory.isEmpty ? "0" : subCategory
                case .douyu, .huya:
                    parentId = nil
                    categoryId = subCategory.isEmpty ? category : subCategory
                }
                result = try await liveKit.getRooms(
                    site: platform,
                    categoryId: categoryId,
                    parentId: parentId,
                    page: page
                )
            }
            guard !Task.isCancelled else { return }
            rooms = result.items
            hasMore = result.hasMore
        } catch is CancellationError {
            // 忽略：被新请求取消
        } catch {
            errorMessage = error.localizedDescription
            rooms = []
            hasMore = false
        }
    }
}
