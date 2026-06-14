import XCTest
@testable import ChaosSeedApp

/// MBGA CDN 排序测试，对齐 Rust `livestream/util/mbga.rs` 的 `mirror_beats_mcdn`。
final class MbgaTests: XCTestCase {

    func testMirrorBeatsMcdn() {
        let urls = [
            "https://foo.mcdn.bilivideo.cn/live-bvc/xx",
            "https://up-gotcha.bilivideo.com/live-bvc/xx",
        ]
        let sorted = Mbga.sortUrls(urls)
        XCTAssertEqual(sorted[0], "https://up-gotcha.bilivideo.com/live-bvc/xx")
        XCTAssertEqual(sorted[1], "https://foo.mcdn.bilivideo.cn/live-bvc/xx")
    }

    func testCdnLevel_knownHosts() {
        // mirror: bilivideo.com 且 host 以 "up" 开头。
        XCTAssertEqual(Mbga.cdnLevel("https://up-gotcha.bilivideo.com/x"), 0)
        // cache: bilivideo.com 但不以 "up" 开头。
        XCTAssertEqual(Mbga.cdnLevel("https://xy123.bilivideo.com/x"), 1)
        // mcdn
        XCTAssertEqual(Mbga.cdnLevel("https://foo.mcdn.bilivideo.cn/x"), 2)
        // pcdn
        XCTAssertEqual(Mbga.cdnLevel("https://foo.szbdyd.com/x"), 3)
        // 无法解析 → mcdn（最低）。
        XCTAssertEqual(Mbga.cdnLevel("not-a-url"), 2)
    }

    func testSortUrls_deduplicatesAndSorts() {
        let urls = [
            "https://b.mcdn.bilivideo.cn/x",
            "https://up.mirror.bilivideo.com/x",
            "https://b.mcdn.bilivideo.cn/x", // 重复
        ]
        let sorted = Mbga.sortUrls(urls)
        XCTAssertEqual(sorted.count, 2)
        XCTAssertEqual(sorted[0], "https://up.mirror.bilivideo.com/x")
        XCTAssertEqual(sorted[1], "https://b.mcdn.bilivideo.cn/x")
    }
}
