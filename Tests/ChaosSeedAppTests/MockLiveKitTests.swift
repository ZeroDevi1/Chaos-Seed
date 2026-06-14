import XCTest
@testable import ChaosSeedApp

/// MockLiveKit 测试，对齐原型 `data.jsx` 的过滤/分页行为，
/// 以及 Rust `LiveDirectoryClient`/`LivestreamClient` 的返回形状。
final class MockLiveKitTests: XCTestCase {
    let kit = MockLiveKit(simulateLatency: false)

    // MARK: 分类

    func testBiliCategoriesHaveTwoLevels() async throws {
        let cats = try await kit.getCategories(site: .biliLive)
        XCTAssertFalse(cats.isEmpty)
        let game = cats.first { $0.id == "game" }
        XCTAssertEqual(game?.name, "网游")
        XCTAssertFalse(game?.children.isEmpty ?? true)
        XCTAssertEqual(game?.children.first?.name, "英雄联盟")
    }

    func testAllSitesHaveRecommendCategory() async throws {
        for site in Site.allCases {
            let cats = try await kit.getCategories(site: site)
            XCTAssertEqual(cats.first?.id, "recommend", "site \(site.rawKey) should start with recommend")
        }
    }

    // MARK: 推荐 / 分类房间

    func testRecommendReturnsAllRoomsForPlatform() async throws {
        let list = try await kit.getRecommendRooms(site: .biliLive, page: 1)
        XCTAssertFalse(list.items.isEmpty)
        XCTAssertTrue(list.items.allSatisfy { $0.site == .biliLive })
        // input 形如 `bilibili:<rid>`
        XCTAssertTrue(list.items.allSatisfy { $0.input.hasPrefix("bilibili:") })
    }

    func testCategoryRooms_filteredByTags() async throws {
        // bili_live 的 game 分类应只返回 tags 含 "game" 的房间（b3/b5/b7/b8）。
        let list = try await kit.getCategoryRooms(site: .biliLive, parentId: "game", categoryId: "game", page: 1)
        let titles = list.items.map(\.title)
        XCTAssertTrue(titles.contains("炉石传说 竞技场12胜"))
        XCTAssertTrue(titles.contains("CS2 完美S局"))
        XCTAssertFalse(titles.contains("原神 每日+深渊")) // mobile/genshin
    }

    // MARK: 分页

    func testRecommendFirstPageHasMoreWhenTotalExceedsPageSize() async throws {
        // bili_live 有 8 个房间，pageSize=6，第 1 页应 hasMore。
        let p1 = try await kit.getRecommendRooms(site: .biliLive, page: 1)
        XCTAssertEqual(p1.items.count, 6)
        XCTAssertTrue(p1.hasMore)

        let p2 = try await kit.getRecommendRooms(site: .biliLive, page: 2)
        XCTAssertEqual(p2.items.count, 2)
        XCTAssertFalse(p2.hasMore)
    }

    func testPageBeyondRangeReturnsEmpty_noMore() async throws {
        let p3 = try await kit.getRecommendRooms(site: .biliLive, page: 3)
        XCTAssertTrue(p3.items.isEmpty)
        XCTAssertFalse(p3.hasMore)
    }

    // MARK: 搜索

    func testSearch_matchesTitleOrStreamer() async throws {
        let list = try await kit.searchRooms(site: .biliLive, keyword: "原神", page: 1)
        XCTAssertEqual(list.items.count, 1)
        XCTAssertEqual(list.items.first?.userName, "棉花大哥哥")
    }

    func testSearch_caseInsensitiveOnStreamer() async throws {
        let list = try await kit.searchRooms(site: .biliLive, keyword: "uzi", page: 1)
        XCTAssertEqual(list.items.first?.userName, "Uzi")
    }

    func testSearch_emptyKeyword_returnsAll() async throws {
        // 空关键词等价于不过滤，走推荐全量。
        let list = try await kit.searchRooms(site: .huya, keyword: "", page: 1)
        XCTAssertEqual(list.items.count, 6)
    }

    // MARK: Manifest 解析

    func testDecodeManifest_returnsVariantsAlignedWithSample() async throws {
        let manifest = try await kit.decodeManifest(input: "bilibili:123", options: .default)
        XCTAssertEqual(manifest.site, .biliLive)
        XCTAssertEqual(manifest.roomId, "123")
        XCTAssertFalse(manifest.variants.isEmpty)
        XCTAssertTrue(manifest.variants.contains { $0.label == "原画" })
    }

    func testDecodeManifest_acceptsURL() async throws {
        let manifest = try await kit.decodeManifest(input: "https://www.huya.com/123", options: .default)
        XCTAssertEqual(manifest.site, .huya)
    }

    // MARK: 变体解析

    func testResolveVariant_fillsMissingUrl() async throws {
        let variants = MockLiveKit.sampleVariants(site: .biliLive)
        let pending = variants.first { !$0.isResolved }!
        let resolved = try await kit.resolveVariant(site: .biliLive, roomId: "1", variantId: pending.id)
        XCTAssertNotNil(resolved.url)
    }

    func testResolveVariant_unknownId_throws() async {
        do {
            _ = try await kit.resolveVariant(site: .biliLive, roomId: "1", variantId: "nope")
            XCTFail("expected throw")
        } catch {
            // ok
        }
    }

    // MARK: Site.makeInput

    func testMakeInput_format() {
        XCTAssertEqual(Site.biliLive.makeInput(roomId: "100"), "bilibili:100")
        XCTAssertEqual(Site.douyu.makeInput(roomId: "1"), "douyu:1")
        XCTAssertEqual(Site.huya.makeInput(roomId: "555"), "huya:555")
    }

    // MARK: formatViewers

    func testFormatViewers_wanScale() {
        XCTAssertEqual(formatViewers(12_4300), "12.4万")
        XCTAssertEqual(formatViewers(2_100_000), "210.0万")
        XCTAssertEqual(formatViewers(8_720), "8,720")
    }
}
