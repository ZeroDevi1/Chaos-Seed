import XCTest
@testable import ChaosSeedApp

/// 基础编解码测试，对齐 Rust 端 `md5::compute` / base64 行为。
final class CryptoTests: XCTestCase {
    func testMd5Hex_knownVectors() {
        // 标准空串/简单串的 MD5。
        XCTAssertEqual(Crypto.md5Hex(""), "d41d8cd98f00b204e9800998ecf8427e")
        XCTAssertEqual(Crypto.md5Hex("abc"), "900150983cd24fb0d6963f7d28e17f72")
        XCTAssertEqual(Crypto.md5Hex("hello world"), "5eb63bbbe01eeed093cb22bb8f5acdc3")
    }

    func testBase64RoundTrip() {
        let s = "RB1!secret_prefix_value"
        let encoded = Crypto.base64Encode(s)
        XCTAssertEqual(Crypto.base64DecodeToString(encoded), s)
    }

    func testBase64DecodeToString_known() {
        // "hello" → "aGVsbG8="
        XCTAssertEqual(Crypto.base64DecodeToString("aGVsbG8="), "hello")
    }
}
