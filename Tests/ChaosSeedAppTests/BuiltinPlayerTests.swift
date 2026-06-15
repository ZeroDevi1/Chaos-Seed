import XCTest
@testable import ChaosSeedApp

@MainActor
final class BuiltinPlayerTests: XCTestCase {
    func testPlayerIdentityRemainsStableAcrossPlaybackAndStop() {
        let player = BuiltinPlayer()
        let identity = player.avPlayer
        let hints = PlaybackHints()

        player.play(url: URL(string: "https://example.com/first.m3u8")!, hints: hints)
        XCTAssertTrue(player.avPlayer === identity)

        player.play(url: URL(string: "https://example.com/second.m3u8")!, hints: hints)
        XCTAssertTrue(player.avPlayer === identity)

        player.stop()
        XCTAssertTrue(player.avPlayer === identity)
    }

    func testHTTPFLVUsesWebBackendAndPreservesHeaders() throws {
        let player = BuiltinPlayer()
        let source = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.flv?token=1")!,
            engine: .webFLV
        )

        player.play(
            source: source,
            hints: PlaybackHints(referer: "https://www.douyu.com/", userAgent: "ChaosSeedTests")
        )

        XCTAssertEqual(player.engine, .webFLV)
        let request = try XCTUnwrap(player.webFLVRequest)
        XCTAssertEqual(request.url, source.url)
        XCTAssertEqual(request.headers["Referer"], "https://www.douyu.com/")
        XCTAssertEqual(request.headers["User-Agent"], "ChaosSeedTests")
    }

    func testWebFLVResourcesAreBundled() throws {
        let page = try XCTUnwrap(WebFLVPlayerView.playerPageURL)
        XCTAssertTrue(FileManager.default.fileExists(atPath: page.path))

        let vendor = try XCTUnwrap(WebFLVPlayerView.resourceRootURL)
            .appendingPathComponent("Vendor/mpegts.js")
        XCTAssertTrue(FileManager.default.fileExists(atPath: vendor.path))
    }
}
