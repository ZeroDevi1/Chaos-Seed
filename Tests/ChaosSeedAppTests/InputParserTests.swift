import XCTest
@testable import ChaosSeedApp

/// InputParser 测试，覆盖 `chaos_core::danmaku::sites::parse_target_hint` 的关键分支。
final class InputParserTests: XCTestCase {

    func testPlatformPrefix_fullNames() throws {
        XCTAssertEqual(try InputParser.parse("bilibili:123").0, .biliLive)
        XCTAssertEqual(try InputParser.parse("douyu:456").0, .douyu)
        XCTAssertEqual(try InputParser.parse("huya:789").0, .huya)
    }

    func testPlatformPrefix_shortAliases() throws {
        XCTAssertEqual(try InputParser.parse("bili:123").0, .biliLive)
        XCTAssertEqual(try InputParser.parse("bl:123").0, .biliLive)
        XCTAssertEqual(try InputParser.parse("dy:123").0, .douyu)
        XCTAssertEqual(try InputParser.parse("hy:123").0, .huya)
    }

    func testPlatformPrefix_returnsRoomId() throws {
        let (site, rid) = try InputParser.parse("bilibili:12345")
        XCTAssertEqual(site, .biliLive)
        XCTAssertEqual(rid, "12345")
    }

    func testPrefixPrefixWithURL_extractsFirstSegment() throws {
        // `site:https://...` 便捷写法：从 URL 取第一段路径。
        let (site, rid) = try InputParser.parse("bilibili:https://live.bilibili.com/999")
        XCTAssertEqual(site, .biliLive)
        XCTAssertEqual(rid, "999")
    }

    func testBiliLiveURL() throws {
        let (site, rid) = try InputParser.parse("https://live.bilibili.com/12345")
        XCTAssertEqual(site, .biliLive)
        XCTAssertEqual(rid, "12345")
    }

    func testDouyuURL() throws {
        let (site, rid) = try InputParser.parse("https://www.douyu.com/67890")
        XCTAssertEqual(site, .douyu)
        XCTAssertEqual(rid, "67890")
    }

    func testHuyaURL() throws {
        let (site, rid) = try InputParser.parse("https://www.huya.com/ai")
        XCTAssertEqual(site, .huya)
        XCTAssertEqual(rid, "ai")
    }

    func testDouyuTopicURL_isRejected() {
        XCTAssertThrowsError(try InputParser.parse("https://www.douyu.com/topic/xx")) { error in
            guard case LiveKitError.invalidInput = error else {
                XCTFail("expected invalidInput for topic url")
                return
            }
        }
    }

    func testUnsupportedHost() {
        XCTAssertThrowsError(try InputParser.parse("https://www.youtube.com/abc")) { error in
            guard case LiveKitError.unsupportedHost(let host) = error, host == "www.youtube.com" else {
                XCTFail("expected unsupportedHost")
                return
            }
        }
    }

    func testPureDigits_defaultsToBiliLive() throws {
        let (site, rid) = try InputParser.parse("12345")
        XCTAssertEqual(site, .biliLive)
        XCTAssertEqual(rid, "12345")
    }

    func testEmptyInput_throws() {
        XCTAssertThrowsError(try InputParser.parse(""))
        XCTAssertThrowsError(try InputParser.parse("   "))
    }

    func testEmptyRoomIdAfterPrefix_throws() {
        XCTAssertThrowsError(try InputParser.parse("bilibili:")) { error in
            guard case LiveKitError.invalidInput = error else {
                XCTFail("expected invalidInput")
                return
            }
        }
    }

    func testAmbiguousInput_throws() {
        XCTAssertThrowsError(try InputParser.parse("some-random-text")) { error in
            guard case LiveKitError.ambiguousInput = error else {
                XCTFail("expected ambiguousInput")
                return
            }
        }
    }
}
