import XCTest
@testable import ChaosSeedApp

/// 真实接口冒烟测试（默认跳过）。
///
/// 编译标志 `SMOKE` 开启：`swift test -Xswift -DSMOKE`。
/// 这些测试会访问真实平台接口（BiliLive/Douyu/Huya），可能因限流/反爬失败，
/// 因此**默认不纳入常规测试套件**——仅用于本地验证移植是否端到端可用。
final class SmokeLiveKitTests: XCTestCase {

    private var isSmokeEnabled: Bool {
        #if SMOKE
        return true
        #else
        return false
        #endif
    }

    private func skipUnlessSmoke() throws {
        try XCTSkipUnless(isSmokeEnabled, "Set -D SMOKE to run live network smoke tests")
    }

    // MARK: 目录

    func testBili_getCategories_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let cats = try await kit.getCategories(site: .biliLive)
        XCTAssertFalse(cats.isEmpty, "bili should return categories")
        XCTAssertFalse(cats.first?.children.isEmpty ?? true, "at least one category should have children")
    }

    func testBili_recommendRooms_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let list = try await kit.getRecommendRooms(site: .biliLive, page: 1)
        XCTAssertFalse(list.items.isEmpty, "recommend should return rooms")
        XCTAssertTrue(list.items.allSatisfy { $0.input.hasPrefix("bilibili:") })
    }

    func testHuya_recommendRooms_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let list = try await kit.getRecommendRooms(site: .huya, page: 1)
        XCTAssertTrue(list.items.allSatisfy { $0.input.hasPrefix("huya:") })
    }

    // MARK: 解析

    /// 解析一个公开直播间。rid=6 为 B 站官方 Dota2 直播间（长期在播），
    /// 失败时跳过而非报错。
    func testBili_decodeManifest_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        do {
            let manifest = try await kit.decodeManifest(input: "bilibili:6", options: .default)
            XCTAssertEqual(manifest.site, .biliLive)
            XCTAssertFalse(manifest.roomId.isEmpty)
            // 打印解析到的清晰度，便于人工核对。
            print("[SMOKE] bili rid=6 variants=\(manifest.variants.map { "\($0.label)(qn=\($0.quality),url=\($0.url != nil))" })")
        } catch {
            // 打印完整错误，便于诊断失败来源。
            print("[SMOKE] decodeManifest error: \(error)")
            throw error
        }
    }
}
