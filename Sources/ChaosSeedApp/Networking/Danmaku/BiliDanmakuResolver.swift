import Foundation

/// BiliLive 表情元数据，用于把 `emoticon_unique` 还原成播放器内图片弹幕。
public struct DanmakuEmoticon: Sendable, Equatable {
    public let unique: String
    public let url: String
    public let width: Int
    public let height: Int
}

/// 弹幕 WebSocket 建连所需的完整凭据。
public struct DanmakuConnectionInfo: Sendable, Equatable {
    public let site: Site
    public let roomId: String
    public let uid: UInt64
    public let token: String
    public let buvid: String
    public let endpoint: URL
    public let emoticons: [String: DanmakuEmoticon]
    public let huyaYyuid: Int64?
    public let huyaUid: Int64?

    public init(
        site: Site = .biliLive,
        roomId: String,
        uid: UInt64 = 0,
        token: String = "",
        buvid: String = "",
        endpoint: URL,
        emoticons: [String: DanmakuEmoticon] = [:],
        huyaYyuid: Int64? = nil,
        huyaUid: Int64? = nil
    ) {
        self.site = site
        self.roomId = roomId
        self.uid = uid
        self.token = token
        self.buvid = buvid
        self.endpoint = endpoint
        self.emoticons = emoticons
        self.huyaYyuid = huyaYyuid
        self.huyaUid = huyaUid
    }
}

/// 对齐 `chaos-core/danmaku/platforms/bili_live.rs` 的解析阶段。
///
/// 建连前依次解析真实 room id、WBI key、getDanmuInfo token 与表情包。
public struct BiliDanmakuResolver: Sendable {
    private static let userAgent =
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15"
    private static let referer = "https://www.bilibili.com/"

    private let http: HTTPClient
    private let wbi: BiliWbi
    private let liveAPIBase: String

    public init(http: HTTPClient, wbi: BiliWbi, liveAPIBase: String = "https://api.live.bilibili.com") {
        self.http = http
        self.wbi = wbi
        self.liveAPIBase = liveAPIBase.trimmingTrailingSlash
    }

    public func resolve(roomId: String) async throws -> DanmakuConnectionInfo {
        let shortId = roomId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard UInt64(shortId) != nil else {
            throw LiveKitError.invalidInput("invalid bilibili room id: \(roomId)")
        }

        let rid = try await fetchCanonicalRoomId(shortId)
        let keys = try await fetchWbiKeys()
        let mixin = try BiliWbi.mixinKey(origin64: keys.imgKey + keys.subKey)
        let signed = BiliWbi.signQuery(
            [("id", rid), ("type", "0"), ("web_location", "444.8")],
            mixinKey: mixin,
            nowS: Int64(Date().timeIntervalSince1970)
        )

        var headers = Self.headers
        if let cookie = BiliAccountStore.combinedCookie(buvidCookie: await wbi.ensureBuvidCookie()) {
            headers["Cookie"] = cookie
        }
        let info = try await http.getJSON(
            "\(liveAPIBase)/xlive/web-room/v1/index/getDanmuInfo",
            query: signed,
            headers: headers
        )
        guard (info.pointer("/code")?.asInt64 ?? 0) == 0 else {
            throw LiveKitError.parse(
                "bili danmaku token error: \(info.pointer("/message")?.asString ?? "unknown")"
            )
        }
        guard let token = info.pointer("/data/token")?.asString?
            .trimmingCharacters(in: .whitespacesAndNewlines),
              !token.isEmpty else {
            throw LiveKitError.parse("bili danmaku token missing")
        }

        let endpoint = Self.pickEndpoint(info) ??
            URL(string: "wss://broadcastlv.chat.bilibili.com/sub")!
        let emoticons = (try? await fetchEmoticons(roomId: rid, headers: headers)) ?? [:]
        return DanmakuConnectionInfo(
            roomId: rid,
            uid: 0,
            token: token,
            buvid: Self.makeBuvid(),
            endpoint: endpoint,
            emoticons: emoticons
        )
    }

    private func fetchCanonicalRoomId(_ roomId: String) async throws -> String {
        let json = try await http.getJSON(
            "\(liveAPIBase)/room/v1/Room/get_info",
            query: [("room_id", roomId)],
            headers: Self.headers
        )
        if let value = json.pointer("/data/room_id")?.asString, !value.isEmpty {
            return value
        }
        if let value = json.pointer("/data/room_id")?.asInt64, value > 0 {
            return String(value)
        }
        throw LiveKitError.parse("bili danmaku canonical room id missing")
    }

    private func fetchWbiKeys() async throws -> BiliWbi.WbiKeys {
        if let cached = wbi.cachedKeys() {
            return cached
        }
        let keys = try await wbi.fetchKeys()
        wbi.setKeys(keys)
        return keys
    }

    private func fetchEmoticons(
        roomId: String,
        headers: [String: String]
    ) async throws -> [String: DanmakuEmoticon] {
        let json = try await http.getJSON(
            "\(liveAPIBase)/xlive/web-ucenter/v2/emoticon/GetEmoticons",
            query: [("platform", "pc"), ("room_id", roomId)],
            headers: headers
        )
        var result: [String: DanmakuEmoticon] = [:]
        for package in json.pointer("/data/data")?.asArray ?? [] {
            let isEmojiPackage = package.pointer("/pkg_name")?.asString == "emoji"
            for value in package.pointer("/emoticons")?.asArray ?? [] {
                guard let unique = value.pointer("/emoticon_unique")?.asString, !unique.isEmpty,
                      let rawURL = value.pointer("/url")?.asString,
                      let url = Self.ensureHTTPS(rawURL), !url.isEmpty else {
                    continue
                }
                let rawWidth = Int(value.pointer("/width")?.asInt64 ?? 0)
                let rawHeight = Int(value.pointer("/height")?.asInt64 ?? 0)
                result[unique] = DanmakuEmoticon(
                    unique: unique,
                    url: url,
                    width: isEmojiPackage ? 75 : rawWidth,
                    height: isEmojiPackage ? 75 : rawHeight
                )
            }
        }
        return result
    }

    private static var headers: [String: String] {
        ["Referer": referer, "User-Agent": userAgent]
    }

    private static func pickEndpoint(_ json: JSONValue) -> URL? {
        for host in json.pointer("/data/host_list")?.asArray ?? [] {
            guard let hostname = host.pointer("/host")?.asString, !hostname.isEmpty else {
                continue
            }
            let port = host.pointer("/wss_port")?.asInt64 ?? 443
            if let url = URL(string: "wss://\(hostname):\(port)/sub") {
                return url
            }
        }
        return nil
    }

    private static func makeBuvid() -> String {
        let compactUUID = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        return "\(compactUUID)\(UInt32.random(in: 10_000...89_999))infoc"
    }

    static func ensureHTTPS(_ value: String) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("https://") {
            return trimmed
        }
        if trimmed.hasPrefix("http://") {
            return "https://" + trimmed.dropFirst("http://".count)
        }
        if trimmed.hasPrefix("//") {
            return "https:\(trimmed)"
        }
        return trimmed.isEmpty ? nil : trimmed
    }
}
