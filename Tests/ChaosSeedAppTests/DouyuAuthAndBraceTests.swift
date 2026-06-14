import XCTest
@testable import ChaosSeedApp

/// Douyu 加密 auth + brace_extract 测试。
final class DouyuAuthAndBraceTests: XCTestCase {

    // MARK: DouyuEncryption.auth

    func testAuth_isDeterministicForSameInput() {
        let enc = DouyuEncryption(key: "KEY", randStr: "RAND", encTime: 3, encData: "DATA", isSpecial: 0)
        let a = enc.auth(rid: "12345", ts: 1700000000)
        let b = enc.auth(rid: "12345", ts: 1700000000)
        XCTAssertEqual(a, b)
        XCTAssertEqual(a.count, 32)
    }

    func testAuth_encTimeZero_singleMd5OfRandPlusKeyPlusRidTs() {
        // encTime=0: u = randStr (循环 0 次), o = rid+ts
        // auth = md5(randStr + key + rid + ts)
        let enc = DouyuEncryption(key: "K", randStr: "R", encTime: 0, encData: "", isSpecial: 0)
        let expected = Crypto.md5Hex("R" + "K" + "123" + "1700000000")
        XCTAssertEqual(enc.auth(rid: "123", ts: 1_700_000_000), expected)
    }

    func testAuth_specialOmitsRidTs() {
        let enc = DouyuEncryption(key: "K", randStr: "R", encTime: 0, encData: "", isSpecial: 1)
        // isSpecial=1: o = ""，auth = md5(randStr + key)
        XCTAssertEqual(enc.auth(rid: "123", ts: 1_700_000_000), Crypto.md5Hex("R" + "K"))
    }

    func testAuth_encTimeIterates() {
        // encTime=2: u = md5(md5("R"+"K")+"K")，再 md5(u + "K" + rid+ts)
        let enc = DouyuEncryption(key: "K", randStr: "R", encTime: 2, encData: "", isSpecial: 0)
        var u = "R"
        u = Crypto.md5Hex(u + "K")
        u = Crypto.md5Hex(u + "K")
        let expected = Crypto.md5Hex(u + "K" + "123" + "1700000000")
        XCTAssertEqual(enc.auth(rid: "123", ts: 1_700_000_000), expected)
    }

    // MARK: BraceExtract

    func testExtractBalancedObject_simple() {
        let input = #"prefix {"a":1,"b":{"c":2}} tail"#
        let obj = BraceExtract.extractBalancedObject(in: input)
        XCTAssertEqual(obj, #"{"a":1,"b":{"c":2}}"#)
    }

    func testExtractBalancedObject_handlesBracesInStrings() {
        let input = #"x {"k":"v{a}l","n":2}"#
        let obj = BraceExtract.extractBalancedObject(in: input)
        XCTAssertEqual(obj, #"{"k":"v{a}l","n":2}"#)
    }

    func testExtractBalancedObject_handlesEscapedQuotes() {
        let input = #"x {"k":"a\"b","n":1}"#
        let obj = BraceExtract.extractBalancedObject(in: input)
        XCTAssertEqual(obj, #"{"k":"a\"b","n":1}"#)
    }

    func testExtractBalancedObject_afterMarker() {
        let input = #"garbage "roomInfo":{"room":{"room_id":7}} more"#
        let obj = BraceExtract.extractBalancedObject(afterMarker: "\"roomInfo\"", in: input)
        XCTAssertNotNil(obj)
        XCTAssertEqual(obj, #"{"room":{"room_id":7}}"#)
    }

    func testExtractBalancedObject_unbalanced_returnsNil() {
        let input = #"{"a":1"#
        let obj = BraceExtract.extractBalancedObject(in: input)
        XCTAssertNil(obj)
    }

    func testExtractBalancedObject_noBrace_returnsNil() {
        XCTAssertNil(BraceExtract.extractBalancedObject(in: "no braces here"))
    }
}
