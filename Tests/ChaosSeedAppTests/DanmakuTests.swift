import Compression
import XCTest
@testable import ChaosSeedApp

final class DanmakuTests: XCTestCase {
    func testConfigDecodesOlderStoredShape() throws {
        let data = Data(#"{"fontSize":24,"blockedWords":["spam"]}"#.utf8)
        let config = try JSONDecoder().decode(DanmakuConfig.self, from: data)

        XCTAssertEqual(config.fontSize, 24)
        XCTAssertEqual(config.blockedWords, ["spam"])
        XCTAssertTrue(config.collapseDuplicates)
        XCTAssertEqual(config.mode, .scroll)
        XCTAssertTrue(config.showScrolling)
        XCTAssertTrue(config.showTop)
        XCTAssertTrue(config.showBottom)
    }

    @MainActor
    func testParseTextCommentKeepsColorModeAndOpacity() throws {
        var metadata = Array<Any>(repeating: 0, count: 16)
        metadata[1] = 5
        metadata[3] = NSNumber(value: UInt32(0x80_FF_00_20))
        let json: [String: Any] = [
            "cmd": "DANMU_MSG",
            "info": [metadata, "hello", [0, "alice"]],
        ]

        let comment = try XCTUnwrap(DanmakuClient.parseComment(json: json))
        XCTAssertEqual(comment.text, "hello")
        XCTAssertEqual(comment.user, "alice")
        XCTAssertEqual(comment.sourceMode, .top)
        XCTAssertEqual(comment.colorRGB, 0xFF_00_20)
        XCTAssertEqual(comment.opacity, 128.0 / 255.0, accuracy: 0.001)
    }

    @MainActor
    func testParseEmoticonFallsBackToResolvedMetadata() throws {
        var metadata = Array<Any>(repeating: 0, count: 16)
        metadata[15] = [
            "extra": #"{"emoticon_unique":"room_emoji","content":"[笑]"}"#,
        ]
        let json: [String: Any] = [
            "cmd": "DANMU_MSG",
            "info": [metadata, "[笑]", [0, "bob"]],
        ]
        let emoticon = DanmakuEmoticon(
            unique: "room_emoji",
            url: "https://example.com/emoji.png",
            width: 180,
            height: 180
        )

        let comment = try XCTUnwrap(DanmakuClient.parseComment(
            json: json,
            emoticons: ["room_emoji": emoticon]
        ))
        XCTAssertTrue(comment.isEmoticon)
        XCTAssertEqual(comment.imageUrl, emoticon.url)
        XCTAssertEqual(comment.imageWidth, 90)
    }

    @MainActor
    func testParseDMV2TextFallback() throws {
        let message = protoField(6, string: "dm v2 text") + protoField(11, varint: 0)
        let json: [String: Any] = ["dm_v2": message.base64EncodedString()]

        let comment = try XCTUnwrap(DanmakuClient.parseComment(json: json))
        XCTAssertEqual(comment.text, "dm v2 text")
    }

    @MainActor
    func testParseDMV2EmoticonFallback() throws {
        let emoticon = protoField(2, string: "//example.com/e.png") +
            protoField(7, varint: 180)
        let entry = protoField(2, message: emoticon)
        let message = protoField(13, varint: 1) + protoField(14, message: entry)
        let json: [String: Any] = ["dm_v2": message.base64EncodedString()]

        let comment = try XCTUnwrap(DanmakuClient.parseComment(json: json))
        XCTAssertEqual(comment.imageUrl, "https://example.com/e.png")
        XCTAssertEqual(comment.imageWidth, 90)
    }

    func testBlockRulesSupportKeywordAndRegex() {
        let comment = DanmakuComment(text: "Hello 123")
        XCTAssertTrue(DanmakuLayoutEngine.isBlocked(comment, rules: ["hello"]))
        XCTAssertTrue(DanmakuLayoutEngine.isBlocked(comment, rules: [#"/\d{3}/"#]))
        XCTAssertFalse(DanmakuLayoutEngine.isBlocked(comment, rules: [#"/^world/"#]))
        XCTAssertTrue(DanmakuLayoutEngine.isValidBlockRule(#"/foo.+bar/"#))
        XCTAssertFalse(DanmakuLayoutEngine.isValidBlockRule(#"/[/"#))
    }

    @MainActor
    func testExtractJSONObjectsSupportsConcatenatedPayloads() {
        let text = #"{"cmd":"A","data":{"text":"} escaped \" quote"}}{"cmd":"B"}"#
        let objects = DanmakuClient.extractJSONObjects(text)
        XCTAssertEqual(objects.count, 2)
        XCTAssertTrue(objects[0].contains(#""cmd":"A""#))
        XCTAssertTrue(objects[1].contains(#""cmd":"B""#))
    }

    func testLayoutCollapsesDuplicatesAndExpiresScrollingItems() throws {
        let start = Date(timeIntervalSinceReferenceDate: 1_000)
        let comments = [
            DanmakuComment(text: "same", receivedAt: start),
            DanmakuComment(text: "same", receivedAt: start.addingTimeInterval(1)),
        ]
        var config = DanmakuConfig.default
        config.fontSize = 18
        config.speed = 1
        config.collapseDuplicates = true

        let visible = DanmakuLayoutEngine.layout(
            comments: comments,
            config: config,
            size: CGSize(width: 800, height: 450),
            now: start.addingTimeInterval(2)
        )
        XCTAssertEqual(visible.count, 1)
        XCTAssertTrue(try XCTUnwrap(visible.first).text.contains("×2"))

        let expired = DanmakuLayoutEngine.layout(
            comments: comments,
            config: config,
            size: CGSize(width: 800, height: 450),
            now: start.addingTimeInterval(20)
        )
        XCTAssertTrue(expired.isEmpty)
    }

    func testLayoutAppliesOpacityThresholdAndColorToggle() throws {
        let start = Date()
        let comment = DanmakuComment(
            text: "color",
            receivedAt: start,
            colorRGB: 0x12_34_56,
            opacity: 0.4
        )
        var config = DanmakuConfig.default
        config.minOpacity = 0.5
        XCTAssertTrue(DanmakuLayoutEngine.layout(
            comments: [comment],
            config: config,
            size: CGSize(width: 640, height: 360),
            now: start
        ).isEmpty)

        config.minOpacity = 0
        config.showColored = false
        let item = try XCTUnwrap(DanmakuLayoutEngine.layout(
            comments: [comment],
            config: config,
            size: CGSize(width: 640, height: 360),
            now: start
        ).first)
        XCTAssertEqual(item.colorRGB, 0xFF_FF_FF)
        XCTAssertEqual(item.opacity, config.opacity * 0.4, accuracy: 0.001)
    }

    func testLayoutKeepsSourceModesAndTypeFilters() {
        let start = Date()
        let comments = [
            DanmakuComment(text: "scroll", receivedAt: start, sourceMode: .scroll),
            DanmakuComment(text: "top", receivedAt: start, sourceMode: .top),
            DanmakuComment(text: "bottom", receivedAt: start, sourceMode: .bottom),
        ]
        var config = DanmakuConfig.default

        XCTAssertEqual(
            Set(DanmakuLayoutEngine.layout(
                comments: comments,
                config: config,
                size: CGSize(width: 800, height: 450),
                now: start
            ).map(\.text)),
            Set(["scroll", "top", "bottom"])
        )

        config.showTop = false
        let filtered = DanmakuLayoutEngine.layout(
            comments: comments,
            config: config,
            size: CGSize(width: 800, height: 450),
            now: start
        )
        XCTAssertFalse(filtered.contains(where: { $0.text == "top" }))
        XCTAssertTrue(filtered.contains(where: { $0.text == "scroll" }))
        XCTAssertTrue(filtered.contains(where: { $0.text == "bottom" }))
    }

    @MainActor
    func testDouyuPacketRoundTripAndCommentParsing() throws {
        let first = DanmakuClient.encodeDouyuPacket("type@=loginreq/roomid@=1/")
        let second = DanmakuClient.encodeDouyuPacket(
            "type@=chatmsg/dms@=1/nn@=alice/txt@=hello@Sworld/"
        )
        let decoded = DanmakuClient.decodeDouyuPackets(first + second)

        XCTAssertEqual(decoded.count, 2)
        XCTAssertEqual(decoded[0], "type@=loginreq/roomid@=1/")
        let comment = try XCTUnwrap(DanmakuClient.parseDouyuComment(decoded[1]))
        XCTAssertEqual(comment.user, "alice")
        XCTAssertEqual(comment.text, "hello/world")
    }

    @MainActor
    func testHuyaJoinPacketContainsRoomCredentials() throws {
        let packet = DanmakuClient.encodeHuyaJoin(yyuid: 123, uid: 456)
        XCTAssertEqual(try HuyaJCE.int32(packet, tag: 0), 1)
        let payload = try XCTUnwrap(HuyaJCE.bytes(packet, tag: 1))
        XCTAssertEqual(try HuyaJCE.int64(payload, tag: 0), 123)
        XCTAssertEqual(try HuyaJCE.int64(payload, tag: 4), 456)
        XCTAssertEqual(try HuyaJCE.int64(payload, tag: 5), 456)
    }

    func testPlatformRoomMetadataParsing() throws {
        let douyu = #"prefix "roomInfo":{"room":{"room_id":3168536}} suffix"#
        XCTAssertEqual(try PlatformDanmakuResolver.parseDouyuRoomId(douyu), "3168536")

        let huya = """
        <script>window.HNF_GLOBAL_INIT = {"roomInfo":{"tLiveInfo":{"lYyid":1,"lUid":"2"}}};</script>
        """
        let object = try PlatformDanmakuResolver.extractHuyaGlobalInit(huya)
        let json = try JSONValue(parsing: object)
        XCTAssertEqual(json.pointer("/roomInfo/tLiveInfo/lYyid")?.asInt64, 1)
        XCTAssertEqual(json.pointer("/roomInfo/tLiveInfo/lUid")?.asInt64, 2)
    }

    func testRightRailKeepsLatestVisibleComments() {
        let comments = (0..<20).map {
            DanmakuComment(
                text: $0 == 19 ? "blocked" : "message \($0)",
                opacity: $0 == 18 ? 0.1 : 1
            )
        }
        var config = DanmakuConfig.default
        config.minOpacity = 0.5
        config.blockedWords = ["blocked"]

        let visible = DanmakuRightRailModel.visibleComments(comments, config: config)

        XCTAssertLessThanOrEqual(visible.count, 100)
        XCTAssertEqual(visible.last?.text, "message 17")
        XCTAssertFalse(visible.contains(where: { $0.text == "message 18" }))
        XCTAssertFalse(visible.contains(where: { $0.text == "blocked" }))
    }

    func testOverlaySwitchDoesNotControlRightRailVisibility() {
        XCTAssertFalse(DanmakuPresentationPolicy.showsOverlay(isEnabled: false))
        XCTAssertTrue(DanmakuPresentationPolicy.showsRightRail(isExpanded: true))
        XCTAssertFalse(DanmakuPresentationPolicy.showsRightRail(isExpanded: false))
    }

    @MainActor
    func testReconnectDelayUsesBoundedExponentialBackoff() {
        XCTAssertEqual(DanmakuClient.reconnectDelay(forAttempt: 1), 1)
        XCTAssertEqual(DanmakuClient.reconnectDelay(forAttempt: 2), 2)
        XCTAssertEqual(DanmakuClient.reconnectDelay(forAttempt: 3), 4)
        XCTAssertEqual(DanmakuClient.reconnectDelay(forAttempt: 8), 15)
    }

    @MainActor
    func testInflateZlibSupportsPayloadLargerThanLegacyEightTimesBuffer() throws {
        let source = Data(String(repeating: #"{"cmd":"DANMU_MSG","info":[]}"#, count: 20_000).utf8)
        let compressed = try compress(source)
        XCTAssertGreaterThan(source.count, compressed.count * 8)
        XCTAssertEqual(DanmakuClient.inflateZlib(compressed), source)
    }

    @MainActor
    func testInflateBiliZlibAcceptsRawDeflateAfterTwoByteWrapper() throws {
        let packet = makePacket(body: Data(#"{"cmd":"DANMU_MSG","info":[]}"#.utf8))
        let raw = try compress(packet)
        let wrapped = Data([0x78, 0x9C]) + raw
        XCTAssertEqual(DanmakuClient.inflateBiliZlib(wrapped), packet)
    }

    @MainActor
    func testInflateBrotli() throws {
        let source = Data(String(repeating: "brotli danmaku ", count: 4_000).utf8)
        let compressed = try compress(source, algorithm: COMPRESSION_BROTLI)
        XCTAssertEqual(DanmakuClient.inflateBrotli(compressed), source)
    }

    private func compress(
        _ source: Data,
        algorithm: compression_algorithm = COMPRESSION_ZLIB
    ) throws -> Data {
        var destination = Data(count: source.count)
        let destinationCount = destination.count
        let written = source.withUnsafeBytes { sourceBuffer in
            destination.withUnsafeMutableBytes { destinationBuffer in
                compression_encode_buffer(
                    destinationBuffer.baseAddress!.assumingMemoryBound(to: UInt8.self),
                    destinationCount,
                    sourceBuffer.baseAddress!.assumingMemoryBound(to: UInt8.self),
                    source.count,
                    nil,
                    algorithm
                )
            }
        }
        guard written > 0 else {
            throw NSError(domain: "DanmakuTests", code: 1)
        }
        return Data(destination.prefix(written))
    }

    private func protoField(_ field: Int, varint: UInt64) -> Data {
        encodeVarint(UInt64(field << 3)) + encodeVarint(varint)
    }

    private func protoField(_ field: Int, string: String) -> Data {
        protoField(field, message: Data(string.utf8))
    }

    private func protoField(_ field: Int, message: Data) -> Data {
        encodeVarint(UInt64((field << 3) | 2)) + encodeVarint(UInt64(message.count)) + message
    }

    private func encodeVarint(_ value: UInt64) -> Data {
        var value = value
        var result = Data()
        repeat {
            var byte = UInt8(value & 0x7F)
            value >>= 7
            if value != 0 { byte |= 0x80 }
            result.append(byte)
        } while value != 0
        return result
    }

    private func makePacket(body: Data) -> Data {
        var packet = Data()
        let length = UInt32(16 + body.count)
        packet.append(UInt8((length >> 24) & 0xFF))
        packet.append(UInt8((length >> 16) & 0xFF))
        packet.append(UInt8((length >> 8) & 0xFF))
        packet.append(UInt8(length & 0xFF))
        packet.append(contentsOf: [0, 16, 0, 0, 0, 0, 0, 5, 0, 0, 0, 1])
        packet.append(body)
        return packet
    }
}
