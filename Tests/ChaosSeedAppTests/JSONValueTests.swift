import XCTest
@testable import ChaosSeedApp

/// JSONValue 指针访问测试，对齐 `serde_json::Value::pointer` 语义。
final class JSONValueTests: XCTestCase {

    func testPointer_objectAndArray() throws {
        let json = try JSONValue(parsing: #"{"data":{"list":[{"roomid":101}]}}"#)
        XCTAssertEqual(json.pointer("/data/list/0/roomid")?.asInt64, 101)
    }

    func testPointer_objectKey() throws {
        let json = try JSONValue(parsing: #"{"a":{"b":"hello"}}"#)
        XCTAssertEqual(json.pointer("/a/b")?.asString, "hello")
    }

    func testAsInt64_fromString() {
        let json = try! JSONValue(parsing: "\"42\"")
        XCTAssertEqual(json.asInt64, 42)
    }

    func testAsInt64_fromBool() {
        let json = try! JSONValue(parsing: "true")
        XCTAssertEqual(json.asInt64, 1)
    }

    func testPointer_missingReturnsNil() throws {
        let json = try JSONValue(parsing: #"{"a":1}"#)
        XCTAssertNil(json.pointer("/b"))
    }

    func testPointer_escapedSlash() throws {
        // serde_json: ~1 表示 "/"
        let json = try JSONValue(parsing: #"{"a/b":1}"#)
        XCTAssertEqual(json.pointer("/a~1b")?.asInt64, 1)
    }

    func testParseNestedArray() throws {
        let json = try JSONValue(parsing: #"[1,[2,[3]]]"#)
        XCTAssertEqual(json.pointer("/1/1/0")?.asInt64, 3)
    }
}
