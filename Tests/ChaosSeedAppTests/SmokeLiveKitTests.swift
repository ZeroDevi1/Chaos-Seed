import XCTest
import AVFoundation
@testable import ChaosSeedApp

/// 真实接口冒烟测试（默认跳过）。
///
/// 编译标志 `SMOKE` 开启：`swift test -Xswift -DSMOKE`。
/// 这些测试会访问真实平台接口（BiliLive/Douyu/Huya），可能因限流/反爬失败，
/// 因此**默认不纳入常规测试套件**——仅用于本地验证移植是否端到端可用。
final class SmokeLiveKitTests: XCTestCase {

    private var biliRoomId: String {
        ProcessInfo.processInfo.environment["BILI_SMOKE_ROOM_ID"] ?? "6"
    }

    private var douyuRoomId: String {
        ProcessInfo.processInfo.environment["DOUYU_SMOKE_ROOM_ID"] ?? "3168536"
    }

    private var huyaRoomId: String {
        ProcessInfo.processInfo.environment["HUYA_SMOKE_ROOM_ID"] ?? "660000"
    }

    private var isSmokeEnabled: Bool {
        #if SMOKE
        return true
        #else
        return false
        #endif
    }

    private func skipUnlessSmoke() throws {
        try XCTSkipUnless(isSmokeEnabled, "Set -D SMOKE to run live network smoke tests")
    }

    // MARK: 目录

    func testBili_getCategories_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let cats = try await kit.getCategories(site: .biliLive)
        XCTAssertFalse(cats.isEmpty, "bili should return categories")
        XCTAssertFalse(cats.first?.children.isEmpty ?? true, "at least one category should have children")
    }

    func testBili_recommendRooms_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let list = try await kit.getRecommendRooms(site: .biliLive, page: 1)
        XCTAssertFalse(list.items.isEmpty, "recommend should return rooms")
        XCTAssertTrue(list.items.allSatisfy { $0.input.hasPrefix("bilibili:") })
        print("[SMOKE] bili recommend=\(list.items.prefix(10).map { "\($0.roomId):\($0.online ?? 0)" })")
    }

    func testHuya_recommendRooms_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let list = try await kit.getRecommendRooms(site: .huya, page: 1)
        XCTAssertTrue(list.items.allSatisfy { $0.input.hasPrefix("huya:") })
    }

    func testDouyu_recommendRooms_live() async throws {
        try skipUnlessSmoke()
        let list = try await RealLiveKit().getRecommendRooms(site: .douyu, page: 1)
        XCTAssertFalse(list.items.isEmpty)
        print("[SMOKE] douyu recommend=\(list.items.prefix(10).map { "\($0.roomId):\($0.online ?? 0)" })")
    }

    // MARK: 解析

    /// 解析一个公开直播间。默认 rid=6，也可通过 BILI_SMOKE_ROOM_ID 指定复现房间。
    func testBili_decodeManifest_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        do {
            let manifest = try await kit.decodeManifest(
                input: "bilibili:\(biliRoomId)",
                options: .default
            )
            XCTAssertEqual(manifest.site, .biliLive)
            XCTAssertFalse(manifest.roomId.isEmpty)
            // 打印解析到的清晰度，便于人工核对。
            let descriptions = manifest.variants.map { variant in
                let formats = variant.allURLs
                    .compactMap { URL(string: $0)?.pathExtension }
                    .filter { !$0.isEmpty }
                return "\(variant.label)(qn=\(variant.quality),formats=\(formats),builtin=\(variant.builtinPlaybackURL != nil))"
            }
            print("[SMOKE] bili rid=\(biliRoomId) variants=\(descriptions)")
        } catch {
            // 打印完整错误，便于诊断失败来源。
            print("[SMOKE] decodeManifest error: \(error)")
            throw error
        }
    }

    /// 验证解析出的 HLS 线路能被 AVFoundation 识别为可播放资产。
    func testBili_builtinAsset_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let manifest = try await kit.decodeManifest(
            input: "bilibili:\(biliRoomId)",
            options: .default
        )
        let variant = try XCTUnwrap(
            manifest.variants.first(where: { $0.builtinPlaybackURL != nil }),
            "应至少解析出一条 HLS 内置播放线路"
        )
        let url = try XCTUnwrap(variant.builtinPlaybackURL)
        let headers = [
            "Referer": manifest.playback.referer ?? "https://live.bilibili.com/",
            "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
        ]
        let asset = AVURLAsset(
            url: url,
            options: ["AVURLAssetHTTPHeaderFieldsKey": headers]
        )
        let isPlayable = try await asset.load(.isPlayable)
        XCTAssertTrue(isPlayable, "AVFoundation 应能加载解析出的 HLS 线路：\(variant.label)")
    }

    func testDouyu_decodeManifest_live() async throws {
        try skipUnlessSmoke()
        let manifest = try await RealLiveKit().decodeManifest(
            input: "douyu:\(douyuRoomId)",
            options: .default
        )
        XCTAssertEqual(manifest.site, .douyu)
        XCTAssertFalse(manifest.variants.isEmpty)
        print("[SMOKE] douyu rid=\(douyuRoomId) variants=\(describe(manifest.variants))")
    }

    func testDouyu_builtinAsset_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let manifest = try await kit.decodeManifest(
            input: "douyu:\(douyuRoomId)",
            options: .default
        )
        let variant = try XCTUnwrap(
            manifest.variants.first(where: { $0.webFLVPlaybackURL != nil }),
            "斗鱼网页接口应返回 HTTP-FLV"
        )
        try await assertHTTPFLVReachable(variant, manifest: manifest)
    }

    func testDouyu_resolvedVariantBuiltinAsset_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let manifest = try await kit.decodeManifest(
            input: "douyu:\(douyuRoomId)",
            options: .default
        )
        let pending = try XCTUnwrap(manifest.variants.first(where: { !$0.isResolved }))
        let resolved = try await kit.resolveVariant(
            site: .douyu,
            roomId: manifest.roomId,
            variantId: pending.id
        )
        XCTAssertEqual(resolved.builtinPlaybackSource?.engine, .libMPV)
        try await assertHTTPFLVReachable(resolved, manifest: manifest)
    }

    func testHuya_decodeManifest_live() async throws {
        try skipUnlessSmoke()
        let manifest = try await RealLiveKit().decodeManifest(
            input: "huya:\(huyaRoomId)",
            options: .default
        )
        XCTAssertEqual(manifest.site, .huya)
        XCTAssertFalse(manifest.variants.isEmpty)
        print("[SMOKE] huya rid=\(huyaRoomId) variants=\(describe(manifest.variants))")
    }

    func testHuya_builtinAsset_live() async throws {
        try skipUnlessSmoke()
        let manifest = try await RealLiveKit().decodeManifest(
            input: "huya:\(huyaRoomId)",
            options: .default
        )
        let variant = try XCTUnwrap(manifest.variants.first(where: { $0.builtinPlaybackURL != nil }))
        let url = try XCTUnwrap(variant.builtinPlaybackURL)
        let asset = AVURLAsset(url: url, options: [
            "AVURLAssetHTTPHeaderFieldsKey": [
                "Referer": manifest.playback.referer ?? "https://www.huya.com/",
                "User-Agent": manifest.playback.userAgent ?? "Mozilla/5.0",
            ],
        ])
        let isPlayable = try await asset.load(.isPlayable)
        XCTAssertTrue(isPlayable)
        try await assertHLSSegmentReachable(variant, manifest: manifest)
    }

    @MainActor
    func testBili_danmakuConnection_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        let connection = try await kit.resolveDanmakuConnection(
            site: .biliLive,
            roomId: biliRoomId
        )
        XCTAssertFalse(connection.roomId.isEmpty)
        XCTAssertFalse(connection.token.isEmpty)
        XCTAssertEqual(connection.endpoint.scheme, "wss")

        let client = DanmakuClient(connection: connection)
        client.connect()
        defer { client.disconnect() }
        for _ in 0..<40 where !client.isConnected {
            try await Task.sleep(nanoseconds: 250_000_000)
        }
        XCTAssertTrue(client.isConnected, client.error ?? "弹幕 WebSocket 未确认进房")
        for _ in 0..<40 where client.comments.isEmpty {
            try await Task.sleep(nanoseconds: 250_000_000)
        }
        XCTAssertFalse(
            client.comments.isEmpty,
            """
            连接成功后 10 秒内未解析出实时弹幕；\
            packets=\(client.receivedPacketCount), json=\(client.jsonMessageCount), \
            last=\(client.lastPacketSummary), jsonShape=\(client.lastJSONSummary), \
            histogram=\(client.packetHistogram), text=\(client.lastTextDiagnostic), \
            inflate=\(client.lastInflateDiagnostic), \
            error=\(client.error ?? "-")
            """
        )
    }

    @MainActor
    func testDouyu_danmakuConnection_live() async throws {
        try skipUnlessSmoke()
        try await assertDanmakuConnection(
            site: .douyu,
            roomId: douyuRoomId,
            requiresComment: false
        )
    }

    @MainActor
    func testHuya_danmakuConnection_live() async throws {
        try skipUnlessSmoke()
        try await assertDanmakuConnection(site: .huya, roomId: huyaRoomId)
    }

    @MainActor
    private func assertDanmakuConnection(
        site: Site,
        roomId: String,
        requiresComment: Bool = true
    ) async throws {
        let connection = try await RealLiveKit().resolveDanmakuConnection(
            site: site,
            roomId: roomId
        )
        let client = DanmakuClient(connection: connection)
        client.connect()
        defer { client.disconnect() }

        for _ in 0..<40 where !client.isConnected {
            try await Task.sleep(nanoseconds: 250_000_000)
        }
        XCTAssertTrue(client.isConnected, client.error ?? "\(site.rawKey) 弹幕未连接")

        if !requiresComment {
            for _ in 0..<12 where client.receivedPacketCount == 0 {
                try await Task.sleep(nanoseconds: 250_000_000)
            }
            XCTAssertGreaterThan(
                client.receivedPacketCount,
                0,
                "\(site.rawKey) 连接后未收到任何业务包"
            )
            return
        }
        for _ in 0..<40 where client.comments.isEmpty {
            try await Task.sleep(nanoseconds: 250_000_000)
        }
        XCTAssertFalse(
            client.comments.isEmpty,
            "\(site.rawKey) 连接后 10 秒内未收到聊天弹幕；packets=\(client.receivedPacketCount), histogram=\(client.packetHistogram), last=\(client.lastPacketSummary), text=\(client.lastTextDiagnostic), error=\(client.error ?? "-")"
        )
    }

    private func assertHTTPFLVReachable(
        _ variant: StreamVariant,
        manifest: LiveManifest
    ) async throws {
        let url = try XCTUnwrap(variant.webFLVPlaybackURL)
        var request = URLRequest(url: url)
        request.setValue(manifest.playback.referer ?? "", forHTTPHeaderField: "Referer")
        request.setValue(manifest.playback.userAgent ?? "Mozilla/5.0", forHTTPHeaderField: "User-Agent")
        let (bytes, response) = try await URLSession.shared.bytes(for: request)
        let http = try XCTUnwrap(response as? HTTPURLResponse)
        XCTAssertTrue((200..<300).contains(http.statusCode))

        var signature: [UInt8] = []
        for try await byte in bytes {
            signature.append(byte)
            if signature.count == 3 { break }
        }
        XCTAssertEqual(String(bytes: signature, encoding: .ascii), "FLV")
    }

    private func assertHLSSegmentReachable(
        _ variant: StreamVariant,
        manifest: LiveManifest
    ) async throws {
        let masterURL = try XCTUnwrap(variant.builtinPlaybackURL)
        let headers = [
            "Referer": manifest.playback.referer ?? "",
            "User-Agent": manifest.playback.userAgent ?? "Mozilla/5.0",
        ]
        let master = try await loadText(masterURL, headers: headers)
        let mediaURL: URL
        if let child = firstMediaReference(in: master),
           let childURL = URL(string: child, relativeTo: masterURL)?.absoluteURL,
           childURL.pathExtension.lowercased() == "m3u8" {
            mediaURL = childURL
        } else {
            mediaURL = masterURL
        }
        let media = mediaURL == masterURL ? master : try await loadText(mediaURL, headers: headers)
        let segment = try XCTUnwrap(firstMediaReference(in: media), "HLS 媒体清单缺少分片")
        let segmentURL = try XCTUnwrap(URL(string: segment, relativeTo: mediaURL)?.absoluteURL)
        var request = URLRequest(url: segmentURL)
        headers.forEach { request.setValue($0.value, forHTTPHeaderField: $0.key) }
        let (_, response) = try await URLSession.shared.data(for: request)
        let http = try XCTUnwrap(response as? HTTPURLResponse)
        XCTAssertTrue((200..<300).contains(http.statusCode), "HLS 分片返回 HTTP \(http.statusCode)")
    }

    private func loadText(_ url: URL, headers: [String: String]) async throws -> String {
        var request = URLRequest(url: url)
        headers.forEach { request.setValue($0.value, forHTTPHeaderField: $0.key) }
        let (data, response) = try await URLSession.shared.data(for: request)
        let http = try XCTUnwrap(response as? HTTPURLResponse)
        XCTAssertTrue((200..<300).contains(http.statusCode))
        return try XCTUnwrap(String(data: data, encoding: .utf8))
    }

    private func firstMediaReference(in playlist: String) -> String? {
        playlist
            .split(whereSeparator: \.isNewline)
            .map(String.init)
            .first { !$0.hasPrefix("#") && !$0.trimmingCharacters(in: .whitespaces).isEmpty }
    }

    private func describe(_ variants: [StreamVariant]) -> [String] {
        variants.map { variant in
            let formats = variant.allURLs.compactMap { URL(string: $0)?.pathExtension }
            let hosts = variant.allURLs.compactMap { URL(string: $0)?.host }
            return "\(variant.label)(quality=\(variant.quality),formats=\(formats),hosts=\(hosts),builtin=\(variant.builtinPlaybackURL != nil))"
        }
    }

    // MARK: 分类房间（复现用户报告的 404）

    /// 先拉分类，再用第一个有子分类的一级分类拉房间列表。
    /// 这是用户报告 "bilibili api error 404" 最可能的发生路径。
    func testBili_categoryRooms_live() async throws {
        try skipUnlessSmoke()
        let kit = RealLiveKit()
        do {
            let cats = try await kit.getCategories(site: .biliLive)
            // 找一个有子分类的一级分类（跳过 recommend）。
            guard let parent = cats.first(where: { !$0.children.isEmpty && $0.id != "recommend" }),
                  let sub = parent.children.first else {
                throw LiveKitError.parse("no category with children")
            }
            print("[SMOKE] bili 分类: parent=\(parent.id)(\(parent.name)) sub=\(sub.id)(\(sub.name))")
            let list = try await kit.getCategoryRooms(site: .biliLive, parentId: parent.id, categoryId: sub.id, page: 1)
            print("[SMOKE] bili 分类房间: \(list.items.count) 个 hasMore=\(list.hasMore)")
            if list.items.isEmpty {
                print("[SMOKE] ⚠️ 分类房间为空（可能被拦截，但未报错）")
            }
        } catch {
            print("[SMOKE] ❌ bili 分类房间失败: \(error)")
            throw error
        }
    }
}
