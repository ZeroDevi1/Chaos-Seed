import Foundation

/// BiliLive WBI 签名 + buvid/access_id 获取与缓存。
///
/// 忠实移植 Rust `chaos_core::live_directory::util::bili_wbi::BiliWbi`。
/// `mixinKey` / `filterWbiValue` / `signQuery` 为纯函数（与 Rust 单元测试对齐）；
/// `fetchKeys` / `fetchBuvid` / `fetchAccessId` 走真实 HTTP。
public final class BiliWbi: @unchecked Sendable {
    /// WBI 签名所需的 img/sub key。
    public struct WbiKeys: Sendable, Equatable {
        public var imgKey: String
        public var subKey: String
    }

    /// buvid3 / buvid4。
    public struct Buvid: Sendable, Equatable {
        public var b3: String
        public var b4: String
    }

    private struct Cached<T> { let value: T; let storedAt: Date }

    private var keys: Cached<WbiKeys>?
    private var accessId: Cached<String>?
    private var buvid: Cached<Buvid>?

    /// API 基址（`https://api.bilibili.com`）。
    public let apiBase: String
    /// live 基址（`https://live.bilibili.com`）。
    public let liveBase: String
    private let http: HTTPClient

    private static let keysTTL: TimeInterval = 6 * 3600
    private static let accessIdTTL: TimeInterval = 24 * 3600
    private static let buvidTTL: TimeInterval = 24 * 3600

    private static let biliUA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    private static let biliReferer = "https://live.bilibili.com/"

    public init(http: HTTPClient, apiBase: String = "https://api.bilibili.com", liveBase: String = "https://live.bilibili.com") {
        self.http = http
        self.apiBase = apiBase
        self.liveBase = liveBase
    }

    // MARK: 纯函数（对齐 Rust 单元测试）

    /// 对齐 `BiliWbi::mixin_key(origin_64)`：用固定置换表取前 32 字符。
    public static func mixinKey(origin64: String) throws -> String {
        let tab: [Int] = [
            46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42,
            19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60,
            51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
        ]
        let chars = Array(origin64.trimmingCharacters(in: .whitespaces))
        guard chars.count >= 64 else {
            throw LiveKitError.invalidInput("origin_64 must be >= 64 chars")
        }
        var out = ""
        out.reserveCapacity(32)
        for i in tab {
            if i < chars.count { out.append(chars[i]) }
        }
        return String(out.prefix(32))
    }

    /// 对齐 `BiliWbi::filter_wbi_value`：过滤 `! ' ( ) *`。
    public static func filterWbiValue(_ v: String) -> String {
        let blocked: Set<Character> = ["!", "'", "(", ")", "*"]
        return String(v.filter { !blocked.contains($0) })
    }

    /// 对齐 `BiliWbi::sign_query(params, mixin_key, now_s)`：
    /// 加入 `wts=now_s`，按键名字典序排序，构造 `k=percent(v)` 拼接，
    /// 计算 `MD5(query + mixin_key)` 作为 `w_rid`。
    public static func signQuery(_ params: [(String, String)], mixinKey: String, nowS: Int64) -> [(String, String)] {
        var map: [String: String] = [:]
        for (k, v) in params { map[k] = v }
        map["wts"] = String(nowS)

        // BTreeMap 按键排序。
        let sortedKeys = map.keys.sorted()
        let pairs = sortedKeys.map { ($0, filterWbiValue(map[$0] ?? "")) }
        let query = pairs
            .map { "\($0.0)=\(Self.percentEncode($0.1))" }
            .joined(separator: "&")
        let signSrc = query + mixinKey
        let wRid = Crypto.md5Hex(signSrc)

        var out: [(String, String)] = []
        for k in sortedKeys { out.append((k, map[k] ?? "")) }
        out.append(("w_rid", wRid))
        return out
    }

    /// 对齐 Rust `urlencoding::encode`：percent-encode 非保留字符。
    /// 与表单编码不同，空格也要编码为 `%20` 而非 `+`。
    private static func percentEncode(_ s: String) -> String {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? s
    }

    // MARK: 缓存（对齐 Rust cached_keys/access_id/buvid 的 TTL 语义）

    public func cachedKeys() -> WbiKeys? {
        guard let c = keys else { return nil }
        guard Date().timeIntervalSince(c.storedAt) < Self.keysTTL,
              !c.value.imgKey.isEmpty, !c.value.subKey.isEmpty else { return nil }
        return c.value
    }
    public func setKeys(_ keys: WbiKeys) { self.keys = Cached(value: keys, storedAt: Date()) }
    public func clearKeys() { keys = nil }

    public func cachedAccessId() -> String? {
        guard let c = accessId, Date().timeIntervalSince(c.storedAt) < Self.accessIdTTL, !c.value.isEmpty else { return nil }
        return c.value
    }
    public func setAccessId(_ id: String) { self.accessId = Cached(value: id, storedAt: Date()) }
    public func clearAccessId() { accessId = nil }

    public func cachedBuvid() -> Buvid? {
        guard let c = buvid,
              Date().timeIntervalSince(c.storedAt) < Self.buvidTTL,
              !c.value.b3.isEmpty, !c.value.b4.isEmpty else { return nil }
        return c.value
    }
    public func setBuvid(_ b: Buvid) { self.buvid = Cached(value: b, storedAt: Date()) }
    public func clearBuvid() { buvid = nil }

    public static func cookie(fromBuvid b: Buvid) -> String {
        "buvid3=\(b.b3);buvid4=\(b.b4);"
    }

    // MARK: 远程获取

    /// 对齐 `fetch_keys`：从 `/x/web-interface/nav` 取 img/sub key。
    public func fetchKeys() async throws -> WbiKeys {
        let url = "\(apiBase)/x/web-interface/nav"
        var headers: [String: String] = [
            "Referer": Self.biliReferer,
            "User-Agent": Self.biliUA,
        ]
        if let cookie = await ensureBuvidCookie() { headers["Cookie"] = cookie }
        let json = try await http.getJSON(url, headers: headers)
        let imgUrl = json.pointer("/data/wbi_img/img_url")?.asString ?? ""
        let subUrl = json.pointer("/data/wbi_img/sub_url")?.asString ?? ""
        let imgKey = keyFromUrl(imgUrl)
        let subKey = keyFromUrl(subUrl)
        guard !imgKey.isEmpty, !subKey.isEmpty else {
            throw LiveKitError.parse("bili wbi keys missing")
        }
        return WbiKeys(imgKey: imgKey, subKey: subKey)
    }

    /// 对齐 `fetch_buvid`：从 `/x/frontend/finger/spi` 取 b_3 / b_4。
    public func fetchBuvid() async throws -> Buvid {
        let url = "\(apiBase)/x/frontend/finger/spi"
        let json = try await http.getJSON(url, headers: [
            "Referer": Self.biliReferer,
            "User-Agent": Self.biliUA,
        ])
        let b3 = json.pointer("/data/b_3")?.asString?.trimmingCharacters(in: .whitespaces) ?? ""
        let b4 = json.pointer("/data/b_4")?.asString?.trimmingCharacters(in: .whitespaces) ?? ""
        guard !b3.isEmpty, !b4.isEmpty else {
            throw LiveKitError.parse("bili buvid missing")
        }
        return Buvid(b3: b3, b4: b4)
    }

    /// 对齐 `fetch_access_id`：从 `/lol` 页面用正则提取 access_id。
    public func fetchAccessId() async throws -> String {
        let url = "\(liveBase)/lol"
        var headers: [String: String] = [
            "Referer": Self.biliReferer,
            "User-Agent": Self.biliUA,
        ]
        if let cookie = await ensureBuvidCookie() { headers["Cookie"] = cookie }
        let text = try await http.getText(url, headers: headers)
        // 正则 `"access_id":"(.*?)"`，捕获后去除反斜杠转义。
        guard let regex = try? NSRegularExpression(pattern: #""access_id":"(.*?)""#, options: []),
              let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)),
              let r = Range(match.range(at: 1), in: text) else {
            return ""
        }
        return String(text[r]).replacingOccurrences(of: "\\", with: "")
    }

    /// 对齐 `ensure_buvid_cookie`：取缓存或远端，组装 Cookie 字符串。
    /// 注意：失败时缓存空串以避免重复请求（Rust 语义）——这里失败直接返回 nil。
    public func ensureBuvidCookie() async -> String? {
        if let cached = cachedBuvid() { return Self.cookie(fromBuvid: cached) }
        guard let fetched = try? await fetchBuvid() else { return nil }
        setBuvid(fetched)
        return Self.cookie(fromBuvid: fetched)
    }

    /// 从 `https://i0.hdslb.com/<KEY>.png` 风格 URL 取文件名（去扩展名）。
    private func keyFromUrl(_ url: String) -> String {
        let trimmed = url.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return "" }
        let last = trimmed.split(separator: "/").last.map(String.init) ?? trimmed
        return last.split(separator: ".").first.map(String.init) ?? last
    }
}
