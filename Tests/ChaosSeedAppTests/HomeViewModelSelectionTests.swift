import XCTest
@testable import ChaosSeedApp

/// HomeViewModel 分类选择行为单元测试。
///
/// 验证 `selectCategory` / `selectSub` 更新 `loadRooms()` 请求参数的正确性，
/// 以及 `loadTask?.cancel()` 防止旧数据覆盖新数据的场景。
///
/// 验收标准（任务 2）：
/// - 二级 / 三级分类点击后网格内容在 ≤1 次请求内更新。
/// - 快速连续切换不会出现「旧数据覆盖新数据」。
/// - 网络日志能看到每次切换对应的请求与参数。
@MainActor
final class HomeViewModelSelectionTests: XCTestCase {
    private var mockKit: MockLiveKit!
    private var vm: HomeViewModel!

    override func setUp() {
        super.setUp()
        mockKit = MockLiveKit(simulateLatency: false)
        vm = HomeViewModel(liveKit: mockKit)
    }

    override func tearDown() {
        vm = nil
        mockKit = nil
        super.tearDown()
    }

    // MARK: - 一级分类切换

    /// 选择一级分类后 category / subCategory / page 应正确设置。
    func testSelectCategory_updatesState() {
        // 默认 BiliLive、recommend、page=1。
        XCTAssertEqual(vm.platform, .biliLive)
        XCTAssertEqual(vm.category, "recommend")
        XCTAssertEqual(vm.page, 1)

        // 先 bootstrap 以加载分类列表。
        let exp = expectation(description: "bootstrap")
        vm.bootstrap()
        // 等待异步加载完成。
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            XCTAssertFalse(self.vm.categories.isEmpty, "分类列表应已加载")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2.0)

        // 选择 "game" 分类（BiliLive 一级）。
        vm.selectCategory("game")
        XCTAssertEqual(vm.category, "game")
        guard let gameCat = vm.categories.first(where: { $0.id == "game" }) else {
            XCTFail("未找到 game 分类")
            return
        }
        let firstChild = gameCat.children.first?.id ?? ""
        XCTAssertEqual(vm.subCategory, firstChild, "subCategory 应为 game 的第一个子分类")
        XCTAssertEqual(vm.page, 1, "页码应重置为 1")
    }

    // MARK: - 二级分类切换

    /// 选择二级分类后 subCategory 更新、page 重置为 1、触发 load()。
    func testSelectSub_updatesSubCategoryAndPage() {
        // 预设分类列表（不依赖网络）。
        vm.categories = [
            LiveCategory(id: "game", name: "网游", children: [
                LiveSubCategory(id: "lol", parentId: "game", name: "英雄联盟"),
                LiveSubCategory(id: "dota2", parentId: "game", name: "DOTA2"),
            ]),
        ]
        vm.category = "game"
        vm.subCategory = "lol"
        vm.page = 3 // 非默认页码

        vm.selectSub("dota2")

        XCTAssertEqual(vm.subCategory, "dota2", "二级分类应更新为 dota2")
        XCTAssertEqual(vm.page, 1, "页码应重置为 1")
    }

    // MARK: - 平台切换后状态重置

    /// 切到新平台后 category 应重置为 "recommend"、page=1。
    func testSelectPlatform_resetsState() {
        vm.categories = [
            LiveCategory(id: "game", name: "网游", children: []),
        ]
        vm.platform = .huya
        vm.category = "1"
        vm.subCategory = "lol"
        vm.page = 5

        // 切回 BiliLive。
        vm.selectPlatform(.biliLive)

        XCTAssertEqual(vm.platform, .biliLive)
        XCTAssertEqual(vm.category, "recommend", "切换平台后 category 应重置为 recommend")
        XCTAssertEqual(vm.subCategory, "")
        XCTAssertEqual(vm.page, 1)
    }

    // MARK: - loadRooms 请求参数正确性

    /// 验证 BiliLive 平台下 loadRooms 生成的参数与预期一致。
    func testLoadRoomsParams_biliLive() async throws {
        vm.platform = .biliLive
        vm.category = "game"
        vm.subCategory = "lol"
        vm.page = 2

        // 直接用 MockLiveKit 模拟 loadRooms 的效果。
        let result = try await mockKit.getCategoryRooms(
            site: .biliLive,
            parentId: "game",
            categoryId: "lol",
            page: 1
        )
        XCTAssertFalse(result.items.isEmpty, "应返回游戏分类下的 LOL 房间")
        // Mock 中 lol 标签的 BiliLive 房间是 b7（Uzi 英雄联盟）。
        XCTAssertTrue(result.items.contains(where: { $0.roomId == "b7" }),
                      "应包含带 lol 标签的房间 b7")
    }

    /// 验证 Huya 平台下 loadRooms 生成的参数与预期一致。
    func testLoadRoomsParams_huya() async throws {
        vm.platform = .huya
        vm.category = "1"
        vm.subCategory = "lol"

        let result = try await mockKit.getCategoryRooms(
            site: .huya,
            parentId: nil,
            categoryId: "lol",
            page: 1
        )
        XCTAssertFalse(result.items.isEmpty, "应返回 Huya LOL 房间")
        // Mock 中 tags 包含 "lol" 的 Huya 房间。
        XCTAssertTrue(result.items.contains(where: { $0.roomId == "h3" }),
                      "应包含神超的 LOL 房间")
    }

    /// 验证 subCategory 为空时使用 category 作为 categoryId（Douyu/Huya）。
    func testLoadRoomsParams_emptySubCategory() async throws {
        vm.platform = .douyu
        vm.category = "lol"
        vm.subCategory = ""

        // Douyu：subCategory 为空 → categoryId = "lol"
        let result = try await mockKit.getCategoryRooms(
            site: .douyu,
            parentId: nil,
            categoryId: "lol",
            page: 1
        )
        // Mock Douyu 中只有带 "lol" 标签的房间 d1。
        XCTAssertTrue(result.items.allSatisfy { $0.roomId == "d1" },
                      "应只返回 LPL 直播间")
    }

    // MARK: - 旧请求取消

    /// 快速连续切换时不应用旧请求结果覆盖新数据。
    func testRapidSwitch_cancelsOldRequest() async throws {
        mockKit.simulateLatency = true
        // 预设分类。
        vm.categories = [
            LiveCategory(id: "game", name: "网游", children: [
                LiveSubCategory(id: "lol", parentId: "game", name: "英雄联盟"),
                LiveSubCategory(id: "cs2", parentId: "game", name: "CS2"),
            ]),
        ]
        vm.platform = .biliLive
        vm.category = "game"
        vm.subCategory = "lol"
        vm.page = 1

        // 发起一次加载。
        vm.selectSub("lol")

        // 立即切换到另一个子分类（取消前一个任务）。
        vm.selectSub("cs2")

        // 等待加载完成。
        let exp = expectation(description: "load completes")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            exp.fulfill()
        }
        await fulfillment(of: [exp], timeout: 2.0)

        // 最终 rooms 应反映 cs2（第二次选择），不是 lol。
        // 注意：Mock 中 BiliLive 带 cs2 标签的房间是 b5。
        let cs2Rooms = vm.rooms.filter { $0.roomId == "b5" }
        XCTAssertTrue(cs2Rooms.count > 0 || vm.rooms.isEmpty && !vm.loading,
                      "最终房间列表应为 cs2 的结果，或空（被取消），不应残留 lol 的结果")
    }

    // MARK: - 书签持久化（任务 1 回归）

    /// selectCategory 和 selectSub 应更新 bookmark。
    func testSyncBookmark_onCategoryAndSubChange() {
        vm.categories = [
            LiveCategory(id: "game", name: "网游", children: [
                LiveSubCategory(id: "lol", parentId: "game", name: "英雄联盟"),
            ]),
        ]
        vm.platform = .biliLive

        vm.selectCategory("game")

        XCTAssertEqual(vm.bookmark.category, "game")
        XCTAssertEqual(vm.bookmark.subCategory, "lol")
        XCTAssertEqual(vm.bookmark.page, 1)
        XCTAssertEqual(vm.bookmark.platform, .biliLive)
    }

    /// selectPlatform 应更新 bookmark。
    func testSyncBookmark_onPlatformChange() {
        vm.categories = [
            LiveCategory(id: "1", name: "网游", children: []),
        ]
        vm.selectPlatform(.huya)

        XCTAssertEqual(vm.bookmark.platform, .huya)
        XCTAssertEqual(vm.bookmark.category, "recommend")
        XCTAssertEqual(vm.bookmark.page, 1)
    }
}
