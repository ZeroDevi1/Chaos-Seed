import XCTest
@testable import ChaosSeedApp

/// Huya FLV URL 拼接测试，对齐 Rust `livestream/util/huya_url.rs` 的 `format_includes_required_params`。
final class HuyaUrlTests: XCTestCase {

    /// 复刻 Rust 测试：fm = base64("prefix_$0_$1_$2_$3")，未 percent-encode。
    func testFormat_includesRequiredParams() throws {
        let fmPlain = "prefix_$0_$1_$2_$3"
        let fmB64 = Crypto.base64Encode(fmPlain)
        // base64 中通常无保留字符，percentDecode 原样返回。
        XCTAssertEqual(Crypto.base64DecodeToString(fmB64), fmPlain)

        let anti = "wsTime=67b6c60d&ctype=tars_mp&t=102&fs=1&fm=\(fmB64)"

        let url = try XCTUnwrap(HuyaUrl.format(
            streamName: "s",
            flvUrl: "http://x",
            flvSuffix: "flv",
            flvAntiCode: anti,
            presenterUid: 777,
            nowMs: 1_000,
            ratio: 2000
        ))

        XCTAssertTrue(url.hasPrefix("https://x/s.flv?"), "got: \(url)")
        XCTAssertTrue(url.contains("wsSecret="))
        XCTAssertTrue(url.contains("wsTime=67b6c60d"))
        XCTAssertTrue(url.contains("seqid="))
        XCTAssertTrue(url.contains("ver=1"))
        XCTAssertTrue(url.contains("fs=1"))
        XCTAssertTrue(url.contains("t=102"))
        XCTAssertTrue(url.contains("u="))
        XCTAssertTrue(url.contains("ratio=2000"))
        XCTAssertTrue(url.contains("codec=264"))
    }

    /// 缺少 fm / wsTime 应返回 nil。
    func testFormat_missingRequired_returnsNil() {
        let anti = "ctype=tars_mp&t=102" // 无 fm / wsTime
        XCTAssertNil(HuyaUrl.format(
            streamName: "s",
            flvUrl: "http://x",
            flvSuffix: "flv",
            flvAntiCode: anti,
            presenterUid: 777,
            nowMs: 1_000
        ))
    }

    /// http → https 强制升级。
    func testFormat_httpsUpgrade() throws {
        let fmB64 = Crypto.base64Encode("prefix_$0_$1_$2_$3")
        let anti = "wsTime=abc&ctype=t&fm=\(fmB64)"
        let url = try XCTUnwrap(HuyaUrl.format(
            streamName: "s", flvUrl: "http://x", flvSuffix: "flv",
            flvAntiCode: anti, presenterUid: 1, nowMs: 1
        ))
        XCTAssertTrue(url.hasPrefix("https://"))
    }
}
