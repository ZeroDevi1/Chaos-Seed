import XCTest
@testable import ChaosSeedApp

/// WBI 签名纯函数测试，对齐 Rust `tests/live_directory_mock.rs` 的三个断言。
final class BiliWbiTests: XCTestCase {

    /// Rust: `bili_wbi_mixin_key_matches_known_value`
    /// 输入完整的 64 字符表，期望 `KLi2R8nwfOavW3JzrH5Nx9GjtseDcCFd`（dart 算法预计算值）。
    func testMixinKey_matchesKnownValue() throws {
        // 0-9 + a-z + A-Z + "-_" = 64 字符。
        let origin = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_"
        let mixin = try BiliWbi.mixinKey(origin64: origin)
        XCTAssertEqual(mixin, "KLi2R8nwfOavW3JzrH5Nx9GjtseDcCFd")
        XCTAssertEqual(mixin.count, 32)
    }

    /// Rust: `bili_wbi_filter_value_removes_reserved_chars`
    func testFilterWbiValue_removesReservedChars() {
        XCTAssertEqual(BiliWbi.filterWbiValue("a!b'c(d)e*f)g"), "abcdefg")
        XCTAssertEqual(BiliWbi.filterWbiValue("plain"), "plain")
        XCTAssertEqual(BiliWbi.filterWbiValue(""), "")
    }

    /// Rust: `bili_wbi_sign_query_adds_wts_and_w_rid`
    func testSignQuery_addsWtsAndWRid() {
        let params: [(String, String)] = [
            ("b", "2"),
            ("a", "1!2"),
        ]
        let out = BiliWbi.signQuery(params, mixinKey: "mixin", nowS: 123)
        let map = Dictionary(out, uniquingKeysWith: { a, _ in a })

        XCTAssertEqual(map["wts"], "123")
        let wRid = map["w_rid"]
        XCTAssertNotNil(wRid)
        XCTAssertEqual(wRid?.count, 32)
        XCTAssertTrue(wRid?.allSatisfy { $0.isHexDigitChar } == true)
    }

    func testMixinKey_shortInput_throws() {
        XCTAssertThrowsError(try BiliWbi.mixinKey(origin64: "short")) { error in
            guard case LiveKitError.invalidInput = error else {
                XCTFail("expected invalidInput")
                return
            }
        }
    }
}

private extension Character {
    /// 仅匹配 0-9 / a-f / A-F，避免与系统 `isHexDigit` 递归。
    var isHexDigitChar: Bool {
        ("0"..."9").contains(self) || ("a"..."f").contains(self) || ("A"..."F").contains(self)
    }
}
