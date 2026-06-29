import XCTest
import Combine
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

    func testMultipleSourcesStartWithPreferredFirstCandidate() throws {
        let player = BuiltinPlayer()
        let hls = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.m3u8")!,
            engine: .avFoundation
        )
        let flv = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.flv")!,
            engine: .webFLV
        )

        player.play(sources: [hls, flv], hints: PlaybackHints())

        XCTAssertEqual(player.activeSource, hls)
        XCTAssertEqual(player.activeSourceIndex, 0)
        XCTAssertEqual(player.sourceCount, 2)
        XCTAssertEqual(player.engine, .avFoundation)
    }

    func testVariantBuildsNativeThenLibMPVThenWebFallbackSources() throws {
        let variant = StreamVariant(
            id: "huya:0:原画",
            label: "原画",
            quality: 9_999_999,
            url: "https://cdn.example.com/live.flv",
            backupUrls: [
                "https://cdn.example.com/live.m3u8",
                "https://backup.example.com/live.flv",
            ]
        )

        XCTAssertEqual(variant.builtinPlaybackSources.map(\.engine), [
            .avFoundation,
            .libMPV,
            .libMPV,
            .webFLV,
            .webFLV,
        ])
        XCTAssertEqual(
            variant.builtinPlaybackSources.first?.url.absoluteString,
            "https://cdn.example.com/live.m3u8"
        )
    }

    func testWebFLVResourcesAreBundled() throws {
        let page = try XCTUnwrap(WebFLVPlayerView.playerPageURL)
        XCTAssertTrue(FileManager.default.fileExists(atPath: page.path))

        let vendor = try XCTUnwrap(WebFLVPlayerView.resourceRootURL)
            .appendingPathComponent("Vendor/mpegts.js")
        XCTAssertTrue(FileManager.default.fileExists(atPath: vendor.path))
    }

    func testWebFLVPlayerFiltersForbiddenRequestHeaders() throws {
        let page = try XCTUnwrap(WebFLVPlayerView.playerPageURL)
        let html = try String(contentsOf: page, encoding: .utf8)

        XCTAssertTrue(html.contains("lower !== \"referer\""))
        XCTAssertTrue(html.contains("lower !== \"user-agent\""))
        XCTAssertTrue(html.contains("headers: safeHeaders"))
    }

    func testWebFLVPlayerProvidesStandardAndWebKitPictureInPicture() throws {
        let page = try XCTUnwrap(WebFLVPlayerView.playerPageURL)
        let html = try String(contentsOf: page, encoding: .utf8)

        XCTAssertTrue(html.contains("video.requestPictureInPicture"))
        XCTAssertTrue(html.contains("video.webkitSetPresentationMode"))
        XCTAssertTrue(html.contains("pictureInPictureAvailability"))
        XCTAssertTrue(html.contains("pictureInPictureState"))
    }

    func testWebFLVPlayerUsesOnlySwiftUIControlsAndProvidesPlaybackBridge() throws {
        let page = try XCTUnwrap(WebFLVPlayerView.playerPageURL)
        let html = try String(contentsOf: page, encoding: .utf8)

        XCTAssertFalse(html.contains("<video id=\"video\" controls"))
        XCTAssertTrue(html.contains("function togglePlayback()"))
        XCTAssertTrue(html.contains("function setMuted(muted)"))
        XCTAssertTrue(html.contains("function setVolume(volume)"))
        XCTAssertTrue(html.contains("type: \"playbackState\""))
    }

    func testIINAArgumentsPassBiliHeadersAsMPVOptionsBeforeURL() {
        let args = IINAPlayer.buildCLIArguments(
            url: "https://example.com/live.flv?token=1",
            hints: PlaybackHints(
                referer: "https://live.bilibili.com/",
                userAgent: "ChaosSeedTests"
            )
        )

        XCTAssertEqual(args, [
            "--no-stdin",
            "--mpv-referrer=https://live.bilibili.com/",
            "--mpv-user-agent=ChaosSeedTests",
            "https://example.com/live.flv?token=1",
        ])
    }

    func testIINAArgumentsCanStartPictureInPictureForWebFLVFallback() {
        let args = IINAPlayer.buildCLIArguments(
            url: "https://example.com/live.flv",
            hints: PlaybackHints(referer: "https://live.bilibili.com/"),
            startPictureInPicture: true
        )

        XCTAssertEqual(args, [
            "--no-stdin",
            "--pip",
            "--mpv-referrer=https://live.bilibili.com/",
            "https://example.com/live.flv",
        ])
    }

    func testWebPictureInPictureRegistrationControlsAvailabilityAndAction() {
        let player = BuiltinPlayer()
        let source = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.flv")!,
            engine: .webFLV
        )
        let registrationID = UUID()
        var actionCount = 0

        player.play(source: source, hints: PlaybackHints())
        player.attachWebPlaybackActions(
            id: registrationID,
            actions: WebPlaybackActions(
                togglePlayback: {},
                setMuted: { _ in },
                setVolume: { _ in },
                togglePictureInPicture: { actionCount += 1 }
            )
        )
        player.webPictureInPictureAvailabilityChanged(true, id: registrationID)

        XCTAssertTrue(player.isPictureInPicturePossible)
        player.togglePictureInPicture()
        XCTAssertEqual(actionCount, 1)

        player.webPictureInPictureStateChanged(true, id: registrationID)
        XCTAssertTrue(player.isPictureInPictureActive)

        player.detachWebPlaybackActions(id: UUID())
        XCTAssertTrue(player.isPictureInPicturePossible)
        player.detachWebPlaybackActions(id: registrationID)
        XCTAssertFalse(player.isPictureInPicturePossible)
    }

    func testRepeatedPictureInPictureAvailabilityDoesNotRepublishSameValue() {
        let player = BuiltinPlayer()
        let source = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.flv")!,
            engine: .webFLV
        )
        let registrationID = UUID()

        player.play(source: source, hints: PlaybackHints())
        player.attachWebPlaybackActions(
            id: registrationID,
            actions: WebPlaybackActions(
                togglePlayback: {},
                setMuted: { _ in },
                setVolume: { _ in },
                togglePictureInPicture: {}
            )
        )

        var publicationCount = 0
        let observation = player.objectWillChange.sink {
            publicationCount += 1
        }

        player.webPictureInPictureAvailabilityChanged(true, id: registrationID)
        player.webPictureInPictureAvailabilityChanged(true, id: registrationID)

        XCTAssertEqual(publicationCount, 1)
        withExtendedLifetime(observation) {}
    }

    func testPlaybackConfirmationPublishesOncePerPlaybackSession() throws {
        let player = BuiltinPlayer()
        let source = BuiltinPlaybackSource(
            url: URL(string: "https://example.com/live.flv")!,
            engine: .webFLV
        )

        player.play(source: source, hints: PlaybackHints())
        player.webPlaybackStateChanged(isPlaying: true, isMuted: false, volume: 1)
        let first = try XCTUnwrap(player.playbackConfirmation)

        player.webPlaybackStateChanged(isPlaying: true, isMuted: false, volume: 1)
        XCTAssertEqual(player.playbackConfirmation, first)

        player.play(source: source, hints: PlaybackHints())
        player.webPlaybackStateChanged(isPlaying: true, isMuted: false, volume: 1)
        let second = try XCTUnwrap(player.playbackConfirmation)
        XCTAssertNotEqual(second.sessionID, first.sessionID)
    }

    func testFullScreenPresentationHidesChromeAndCollapsesRightRail() {
        XCTAssertTrue(PlayerPresentationPolicy.showsPageChrome(isFullScreen: false))
        XCTAssertFalse(PlayerPresentationPolicy.showsPageChrome(isFullScreen: true))
        XCTAssertFalse(PlayerPresentationPolicy.rightRailOnEnteringFullScreen())
        XCTAssertTrue(DanmakuPresentationPolicy.showsRightRail(isExpanded: true))
    }

    func testExpandedDanmakuRailConsumesLayoutWidth() {
        XCTAssertEqual(
            DanmakuPresentationPolicy.rightRailWidth(totalWidth: 1_200, isExpanded: true),
            336,
            accuracy: 0.001
        )
        XCTAssertEqual(
            DanmakuPresentationPolicy.videoWidth(
                totalWidth: 1_200,
                isRightRailExpanded: true
            ),
            864,
            accuracy: 0.001
        )
        XCTAssertEqual(
            DanmakuPresentationPolicy.videoWidth(
                totalWidth: 1_200,
                isRightRailExpanded: false
            ),
            1_200
        )
    }
}
