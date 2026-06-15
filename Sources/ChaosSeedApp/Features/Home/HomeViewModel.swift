import SwiftUI

/// 首页视图模型，承载平台/分类/搜索/分页状态与加载逻辑。
///
/// 复刻原型 `HomeView.jsx` 的状态机：
/// - 切换平台 → 重置分类到第一个、页码归 1、清空搜索词；
/// - 切换分类/搜索词/页码 → 触发重新加载；
/// - 搜索时隐藏分类栏，仅保留平台与结果列表。
///
/// 状态记忆：通过 `HomeBookmark` 持久化上次浏览位置（平台/一级/二级/页码），
/// 由设置开关 `rememberEnabled` 控制是否启用。
@MainActor
public final class HomeViewModel: ObservableObject {
    @Published var platform: Site
    @Published var categories: [LiveCategory] = []
    @Published var category: String
    @Published var subCategory: String
    @Published var keyword: String = ""
    @Published var searchKeyword: String = ""
    @Published var page: Int
    @Published var rooms: [LiveRoomCard] = []
    @Published var hasMore: Bool = false
    @Published var loading: Bool = false
    @Published var errorMessage: String?

    /// 当前位置书签（可读写，外部可通过设置页切换记忆开关）。
    /// 读写不触发 `didSet`——保存动作由 `syncBookmark()` 统一控制。
    public private(set) var bookmark: HomeBookmark

    /// 是否启用位置记忆，由设置页 `@AppStorage("rememberBrowsePosition")` 控制。
    /// 在 `saveBookmarkIfNeeded()` / `bootstrap()` 中直接读 UserDefaults 以保持同步。
    private var rememberEnabled: Bool {
        UserDefaults.standard.bool(forKey: "rememberBrowsePosition")
    }

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
    /// 每次加载递增；只有最新代次允许写回 UI，防止不响应取消的旧请求覆盖新结果。
    private var loadGeneration = 0

    /// 初始化：从持久化书签恢复上次浏览位置（若用户开启记忆开关），否则使用默认值 BiliLive / recommend。
    public init(liveKit: LiveKit) {
        self.liveKit = liveKit
        let saved = HomeBookmark.load()
        self.bookmark = saved
        // 直接读 UserDefaults 的 rememberBrowsePosition（设置页 @AppStorage 写入），
        // 避免依赖 SwiftUI 属性包装器在非 View 类中的可用性。
        let remember = UserDefaults.standard.bool(forKey: "rememberBrowsePosition")
        if remember {
            // 启用记忆时恢复上次位置
            self.platform = saved.platform
            self.category = saved.category
            self.subCategory = saved.subCategory
            self.page = saved.page
        } else {
            // 默认值
            self.platform = .biliLive
            self.category = "recommend"
            self.subCategory = ""
            self.page = 1
        }
    }

    /// 暴露给视图层用于 URL 解析等需要直接调用 LiveKit 的场景。
    public var kit: LiveKit { liveKit }

    /// 首次启动：加载分类列表与房间数据。
    ///
    /// 若启用了位置记忆，不重置 category/subCategory（init 时已从书签恢复），
    /// 但仍需加载分类列表以保证 UI 渲染正确。
    func bootstrap() {
        if rememberEnabled {
            loadTask?.cancel()
            loadGeneration += 1
            let generation = loadGeneration
            let requestedPlatform = platform
            loadTask = Task { [weak self] in
                guard let self else { return }
                do {
                    let cats = try await self.liveKit.getCategories(site: requestedPlatform)
                    guard !Task.isCancelled, generation == self.loadGeneration else { return }
                    self.categories = cats
                    self.restoreSelectionIfValid(in: cats)
                    self.syncBookmark()
                    await self.loadRooms(generation: generation)
                } catch {
                    guard !Task.isCancelled, generation == self.loadGeneration else { return }
                    self.errorMessage = error.localizedDescription
                }
            }
        } else {
            loadCategoriesAndRooms()
        }
    }

    // MARK: - 书签持久化

    /// 将当前平台/分类/页码同步到书签，若记忆开关开启则写入 UserDefaults。
    private func saveBookmarkIfNeeded() {
        guard rememberEnabled else { return }
        bookmark.save()
    }

    /// 在每次平台/分类/页码变化后调用：更新书签内存状态，触发布尔值判断的持久化。
    private func syncBookmark() {
        bookmark.platform = platform
        bookmark.category = category
        bookmark.subCategory = subCategory
        bookmark.page = page
        saveBookmarkIfNeeded()
    }

    // MARK: - 用户动作

    func selectPlatform(_ newPlatform: Site) {
        guard newPlatform != platform else { return }
        platform = newPlatform
        category = "recommend"
        subCategory = ""
        keyword = ""
        searchKeyword = ""
        page = 1
        syncBookmark()
        loadCategoriesAndRooms()
    }

    func selectCategory(_ catId: String) {
        searchKeyword = ""
        keyword = ""
        category = catId
        let parent = categories.first(where: { $0.id == catId })
        subCategory = parent?.children.first?.id ?? ""
        page = 1
        syncBookmark()
        load()
    }

    func selectSub(_ subId: String) {
        subCategory = subId
        page = 1
        syncBookmark()
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
        syncBookmark()
        load()
    }

    func refresh() {
        load()
    }

    private func loadCategoriesAndRooms() {
        loadTask?.cancel()
        loadGeneration += 1
        let generation = loadGeneration
        let requestedPlatform = platform
        loadTask = Task { [weak self] in
            guard let self else { return }
            do {
                Log.network.debug("loadCategoriesAndRooms 开始：platform=\(requestedPlatform.displayName)")
                let cats = try await self.liveKit.getCategories(site: requestedPlatform)
                guard !Task.isCancelled, generation == self.loadGeneration else {
                    Log.network.debug("loadCategoriesAndRooms 被取消")
                    return
                }
                self.categories = cats
                self.category = cats.first?.id ?? "recommend"
                self.subCategory = cats.first?.children.first?.id ?? ""
                self.syncBookmark()
                Log.network.debug("loadCategoriesAndRooms 分类数=\(cats.count) 选中=\(self.category)/\(self.subCategory)")
                await self.loadRooms(generation: generation)
            } catch {
                guard !Task.isCancelled, generation == self.loadGeneration else { return }
                Log.network.error("loadCategoriesAndRooms 失败", error: error)
                self.errorMessage = error.localizedDescription
            }
        }
    }

    private func load() {
        loadTask?.cancel()
        loadGeneration += 1
        let generation = loadGeneration
        loadTask = Task { [weak self] in
            await self?.loadRooms(generation: generation)
        }
    }

    private func loadRooms(generation: Int) async {
        let requestedPlatform = platform
        let requestedCategory = category
        let requestedSubCategory = subCategory
        let requestedPage = page
        let requestedSearch = searchKeyword

        loading = true
        errorMessage = nil
        Log.network.debug("loadRooms 开始：platform=\(requestedPlatform.displayName) category=\(requestedCategory) subCategory=\(requestedSubCategory) page=\(requestedPage) search=\(requestedSearch.isEmpty ? "无" : requestedSearch)")
        defer {
            if generation == loadGeneration {
                loading = false
            }
        }
        do {
            let result: LiveRoomList
            if !requestedSearch.isEmpty {
                Log.network.debug("loadRooms → searchRooms keyword=\(requestedSearch) page=\(requestedPage)")
                result = try await liveKit.searchRooms(
                    site: requestedPlatform,
                    keyword: requestedSearch,
                    page: requestedPage
                )
            } else if requestedCategory == "recommend" || requestedCategory.isEmpty {
                Log.network.debug("loadRooms → getRecommendRooms page=\(requestedPage)")
                result = try await liveKit.getRecommendRooms(site: requestedPlatform, page: requestedPage)
            } else {
                // category = 一级分类 id；subCategory = 二级分类 id（空/"0" 表示该一级分类下全部）。
                // BiliLive 需要 parent_area_id（一级）+ area_id（二级，"0"=全部）。
                // Douyu/Huya 不需要 parentId（接口签名里忽略），传 nil 即可。
                let parentId: String?
                let categoryId: String
                switch requestedPlatform {
                case .biliLive:
                    parentId = requestedCategory
                    categoryId = requestedSubCategory.isEmpty ? "0" : requestedSubCategory
                case .douyu, .huya:
                    parentId = nil
                    categoryId = requestedSubCategory.isEmpty ? requestedCategory : requestedSubCategory
                }
                Log.network.debug("loadRooms → getRooms categoryId=\(categoryId) parentId=\(parentId ?? "nil") page=\(requestedPage)")
                result = try await liveKit.getRooms(
                    site: requestedPlatform,
                    categoryId: categoryId,
                    parentId: parentId,
                    page: requestedPage
                )
            }
            guard !Task.isCancelled, generation == loadGeneration else {
                Log.network.debug("loadRooms 被取消：platform=\(requestedPlatform.displayName) category=\(requestedCategory)")
                return
            }
            Log.network.debug("loadRooms 完成：共 \(result.items.count) 个房间 hasMore=\(result.hasMore)")
            rooms = result.items
            hasMore = result.hasMore
        } catch is CancellationError {
            // 忽略：被新请求取消
            Log.network.debug("loadRooms CancellationError（被新请求取消）")
        } catch {
            guard generation == loadGeneration, !Task.isCancelled else { return }
            Log.network.error("loadRooms 失败", error: error)
            errorMessage = error.localizedDescription
            rooms = []
            hasMore = false
        }
    }

    /// 恢复书签时校验分类仍存在；平台分类改版后回退到当前第一项，避免无选中态。
    private func restoreSelectionIfValid(in categories: [LiveCategory]) {
        guard !categories.isEmpty else {
            category = "recommend"
            subCategory = ""
            page = 1
            return
        }
        guard let selected = categories.first(where: { $0.id == category }) else {
            category = categories[0].id
            subCategory = categories[0].children.first?.id ?? ""
            page = 1
            return
        }
        if selected.children.isEmpty {
            subCategory = ""
        } else if !selected.children.contains(where: { $0.id == subCategory }) {
            subCategory = selected.children[0].id
            page = 1
        }
    }
}
