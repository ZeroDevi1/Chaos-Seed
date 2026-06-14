import Foundation

/// 直播目录平台实现（getCategories / recommend / category rooms / search）。
///
/// 忠实移植 Rust `chaos_core::live_directory::platforms::{bili_live,douyu,huya}.rs`。
/// 共享一个 `LiveDirectoryPlatformContext`（http + endpoints + env + biliWbi），
/// 对齐 Rust `&LiveDirectoryClient` 依赖注入。
public struct LiveDirectoryPlatformContext: Sendable {
    public let http: HTTPClient
    public let endpoints: LiveEndpoints
    public let env: EnvConfig
    public let biliWbi: BiliWbi

    public init(http: HTTPClient, endpoints: LiveEndpoints, env: EnvConfig, biliWbi: BiliWbi) {
        self.http = http
        self.endpoints = endpoints
        self.env = env
        self.biliWbi = biliWbi
    }
}

public enum LiveDirectoryPlatforms {
    // MARK: 分发

    public static func getCategories(ctx: LiveDirectoryPlatformContext, site: Site) async throws -> [LiveCategory] {
        switch site {
        case .biliLive: return try await biliGetCategories(ctx)
        case .douyu: return try await douyuGetCategories(ctx)
        case .huya: return try await huyaGetCategories(ctx)
        }
    }

    public static func getRecommendRooms(ctx: LiveDirectoryPlatformContext, site: Site, page: Int) async throws -> LiveRoomList {
        switch site {
        case .biliLive: return try await biliGetRecommendRooms(ctx, page: page)
        case .douyu: return try await douyuGetRecommendRooms(ctx, page: page)
        case .huya: return try await huyaGetRecommendRooms(ctx, page: page)
        }
    }

    public static func getCategoryRooms(
        ctx: LiveDirectoryPlatformContext,
        site: Site,
        parentId: String?,
        categoryId: String,
        page: Int
    ) async throws -> LiveRoomList {
        switch site {
        case .biliLive: return try await biliGetCategoryRooms(ctx, parentId: parentId, categoryId: categoryId, page: page)
        case .douyu: return try await douyuGetCategoryRooms(ctx, categoryId: categoryId, page: page)
        case .huya: return try await huyaGetCategoryRooms(ctx, categoryId: categoryId, page: page)
        }
    }

    public static func searchRooms(ctx: LiveDirectoryPlatformContext, site: Site, keyword: String, page: Int) async throws -> LiveRoomList {
        switch site {
        case .biliLive: return try await biliSearchRooms(ctx, keyword: keyword, page: page)
        case .douyu: return try await douyuSearchRooms(ctx, keyword: keyword, page: page)
        case .huya: return try await huyaSearchRooms(ctx, keyword: keyword, page: page)
        }
    }
}

// MARK: - 共享辅助

/// Bili 风格的 as_id_string：i64/u64/f64/str 兜底转字符串。
func asIDString(_ v: JSONValue?) -> String {
    guard let v else { return "" }
    if let n = v.asInt64 { return String(n) }
    if case .string(let s) = v {
        let t = s.trimmingCharacters(in: .whitespaces)
        return t.isEmpty ? "" : t
    }
    return ""
}

/// Douyu 风格 as_i64：更宽松（数字字符串也解析）。
func asI64(_ v: JSONValue?) -> Int64 {
    guard let v else { return 0 }
    if let n = v.asInt64 { return n }
    if case .string(let s) = v {
        return Int64(s.trimmingCharacters(in: .whitespaces)) ?? 0
    }
    return 0
}

/// 把 `//xxx` / `http://` 风格封面补成 https。
func absCoverBili(_ url: String?) -> String? {
    guard let url, !url.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
    if url.hasPrefix("//") { return "https:" + url }
    return url
}

/// Douyu 热度字符串解析：`2.3万` → 23000，`1234` → 1234。
func parseHotNum(_ s: String) -> Int64 {
    let raw = s.trimmingCharacters(in: .whitespaces)
    if raw.isEmpty { return 0 }
    if raw.hasSuffix("万") {
        let n = raw.dropLast()
        if let f = Double(n) { return Int64(f * 10_000) }
    }
    return Int64(raw) ?? 0
}

// MARK: - BiliLive

private let BILI_DIR_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
private let BILI_DIR_REFERER = "https://live.bilibili.com/"

private func biliGetJSON(ctx: LiveDirectoryPlatformContext, url: String, query: [(String, String)]) async throws -> JSONValue {
    var headers: [String: String] = [
        "Referer": BILI_DIR_REFERER,
        "User-Agent": BILI_DIR_UA,
    ]
    if let cookie = await ctx.biliWbi.ensureBuvidCookie() { headers["Cookie"] = cookie }
    let json = try await ctx.http.getJSON(url, query: query, headers: headers)
    // Bili APIs 通常 HTTP 200 但 code 字段表示业务错误。
    if let code = json.pointer("/code")?.asInt64, code != 0 {
        let msg = json.pointer("/message")?.asString ?? "unknown error"
        throw LiveKitError.parse("bilibili api error (code=\(code)): \(msg)")
    }
    return json
}

private func biliRetryableCode(_ error: Error) -> Int64? {
    guard case LiveKitError.parse(let s) = error else { return nil }
    guard let range = s.range(of: "code=") else { return nil }
    let rest = s[range.upperBound...]
    guard let parenRange = rest.range(of: ")") else { return nil }
    let codeStr = rest[rest.startIndex..<parenRange.lowerBound]
    return Int64(codeStr)
}

private func isRetryableBiliCode(_ code: Int64) -> Bool { code == -352 || code == -412 }

func biliGetCategories(_ ctx: LiveDirectoryPlatformContext) async throws -> [LiveCategory] {
    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let json = try await biliGetJSON(ctx: ctx, url: "\(base)/room/v1/Area/getList", query: [
        ("need_entrance", "1"),
        ("parent_id", "0"),
    ])

    guard let arr = json.pointer("/data")?.asArray else {
        throw LiveKitError.parse("bili categories: missing data")
    }
    var out: [LiveCategory] = []
    for item in arr {
        let id = asIDString(item.pointer("/id"))
        let name = item.pointer("/name")?.asString ?? ""
        let list = item.pointer("/list")?.asArray ?? []
        var subs: [LiveSubCategory] = []
        for sub in list {
            let sid = asIDString(sub.pointer("/id"))
            let sname = sub.pointer("/name")?.asString ?? ""
            let parentId = asIDString(sub.pointer("/parent_id"))
            var pic: String? = nil
            if let p = sub.pointer("/pic")?.asString, !p.trimmingCharacters(in: .whitespaces).isEmpty {
                if let abs = absCoverBili(p) {
                    pic = abs.contains("@") ? abs : "\(abs)@100w.png"
                }
            }
            if sid.isEmpty || sname.isEmpty { continue }
            subs.append(LiveSubCategory(id: sid, parentId: parentId, name: sname, pic: pic))
        }
        if id.isEmpty || name.isEmpty { continue }
        out.append(LiveCategory(id: id, name: name, children: subs))
    }
    return out
}

func biliGetRecommendRooms(_ ctx: LiveDirectoryPlatformContext, page: Int) async throws -> LiveRoomList {
    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let url = "\(base)/xlive/web-interface/v1/second/getListByArea"
    let baseParams: [(String, String)] = [
        ("platform", "web"),
        ("sort", "online"),
        ("page_size", "30"),
        ("page", String(max(1, page))),
    ]

    var lastErr: Error?
    for attempt in 0..<2 {
        do {
            let nowS = ctx.env.nowS()
            let keys: BiliWbi.WbiKeys
            if let cached = ctx.biliWbi.cachedKeys() {
                keys = cached
            } else {
                keys = try await ctx.biliWbi.fetchKeys()
                ctx.biliWbi.setKeys(keys)
            }
            let mixin = try BiliWbi.mixinKey(origin64: keys.imgKey + keys.subKey)
            let signed = BiliWbi.signQuery(baseParams, mixinKey: mixin, nowS: nowS)
            let v = try await biliGetJSON(ctx: ctx, url: url, query: signed)
            return biliParseRoomListFromDataList(v, hasMoreFallback: { !($0).isEmpty })
        } catch {
            let retryable = (biliRetryableCode(error).map(isRetryableBiliCode) ?? false)
            lastErr = error
            if attempt == 0 && retryable {
                ctx.biliWbi.clearKeys()
                ctx.biliWbi.clearAccessId()
                ctx.biliWbi.clearBuvid()
                continue
            }
            break
        }
    }
    throw lastErr ?? LiveKitError.parse("bilibili request failed")
}

func biliGetCategoryRooms(_ ctx: LiveDirectoryPlatformContext, parentId: String?, categoryId: String, page: Int) async throws -> LiveRoomList {
    let pid = (parentId ?? "").trimmingCharacters(in: .whitespaces)
    guard !pid.isEmpty else { throw LiveKitError.invalidInput("missing parent_id") }
    let cid = categoryId.trimmingCharacters(in: .whitespaces)
    guard !cid.isEmpty else { throw LiveKitError.invalidInput("missing category_id") }

    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let url = "\(base)/xlive/web-interface/v1/second/getList"

    var lastErr: Error?
    for attempt in 0..<2 {
        do {
            let nowS = ctx.env.nowS()
            // access_id
            let accessId: String
            if let cached = ctx.biliWbi.cachedAccessId() {
                accessId = cached
            } else {
                let fetched = try await ctx.biliWbi.fetchAccessId()
                ctx.biliWbi.setAccessId(fetched)
                accessId = fetched
            }
            let params: [(String, String)] = [
                ("platform", "web"),
                ("parent_area_id", pid),
                ("area_id", cid),
                ("sort_type", ""),
                ("page", String(max(1, page))),
                ("w_webid", accessId),
            ]
            let keys: BiliWbi.WbiKeys
            if let cached = ctx.biliWbi.cachedKeys() {
                keys = cached
            } else {
                keys = try await ctx.biliWbi.fetchKeys()
                ctx.biliWbi.setKeys(keys)
            }
            let mixin = try BiliWbi.mixinKey(origin64: keys.imgKey + keys.subKey)
            let signed = BiliWbi.signQuery(params, mixinKey: mixin, nowS: nowS)
            let v = try await biliGetJSON(ctx: ctx, url: url, query: signed)
            let hasMore = v.pointer("/data/has_more")?.asInt64 == 1
            let list = v.pointer("/data/list")?.asArray ?? []
            let items = biliBuildCards(from: list, coverField: { item in
                item.pointer("/cover")
            })
            return LiveRoomList(hasMore: hasMore, items: items)
        } catch {
            let retryable = (biliRetryableCode(error).map(isRetryableBiliCode) ?? false)
            lastErr = error
            if attempt == 0 && retryable {
                ctx.biliWbi.clearKeys()
                ctx.biliWbi.clearAccessId()
                ctx.biliWbi.clearBuvid()
                continue
            }
            break
        }
    }
    // 命中拦截时回退到 v3 稳定接口。
    if let e = lastErr, biliRetryableCode(e).map(isRetryableBiliCode) ?? false {
        return try await biliGetCategoryRoomsFallbackV3(ctx: ctx, pid: pid, cid: cid, page: page)
    }
    throw lastErr ?? LiveKitError.parse("bilibili request failed")
}

/// 对齐 Rust `fallback_v3`：`/room/v3/area/getRoomList`。
private func biliGetCategoryRoomsFallbackV3(ctx: LiveDirectoryPlatformContext, pid: String, cid: String, page: Int) async throws -> LiveRoomList {
    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let url = "\(base)/room/v3/area/getRoomList"
    let pageSize = 30
    let v = try await biliGetJSON(ctx: ctx, url: url, query: [
        ("platform", "web"),
        ("parent_area_id", pid),
        ("area_id", cid),
        ("sort_type", "online"),
        ("page", String(max(1, page))),
        ("page_size", String(pageSize)),
    ])
    let count = max(0, v.pointer("/data/count")?.asInt64 ?? 0)
    let list = v.pointer("/data/list")?.asArray ?? []
    let items = biliBuildCards(from: list, coverField: { item in
        item.pointer("/user_cover") ?? item.pointer("/cover") ?? item.pointer("/system_cover")
    })
    let hasMore: Bool
    if count > 0 {
        hasMore = Int64(max(1, page)) * Int64(pageSize) < count
    } else {
        hasMore = !items.isEmpty
    }
    return LiveRoomList(hasMore: hasMore, items: items)
}

/// 从 `data.list` 数组构造房间卡片。`coverField` 指定封面取值字段。
private func biliBuildCards(from list: [JSONValue], coverField: (JSONValue) -> JSONValue?) -> [LiveRoomCard] {
    var items: [LiveRoomCard] = []
    for x in list {
        guard let rid = x.pointer("/roomid")?.asInt64.map(String.init), !rid.isEmpty else { continue }
        let title = x.pointer("/title")?.asString ?? ""
        var cover: String? = nil
        if let c = (coverField(x)?.asString).flatMap(absCoverBili) {
            cover = c.contains("@") ? c : "\(c)@400w.jpg"
        }
        let userName = x.pointer("/uname")?.asString
        let online = x.pointer("/online")?.asInt64
        items.append(LiveRoomCard(site: .biliLive, roomId: rid, input: Site.biliLive.makeInput(roomId: rid), title: title, cover: cover, userName: userName, online: online.map(Int.init)))
    }
    return items
}

private func biliParseRoomListFromDataList(_ v: JSONValue, hasMoreFallback: ([LiveRoomCard]) -> Bool) -> LiveRoomList {
    let list = v.pointer("/data/list")?.asArray ?? []
    let items = biliBuildCards(from: list, coverField: { $0.pointer("/cover") })
    return LiveRoomList(hasMore: hasMoreFallback(items), items: items)
}

func biliSearchRooms(_ ctx: LiveDirectoryPlatformContext, keyword: String, page: Int) async throws -> LiveRoomList {
    let kw = keyword.trimmingCharacters(in: .whitespaces)
    guard !kw.isEmpty else { throw LiveKitError.invalidInput("keyword is empty") }
    let base = ctx.endpoints.biliApiBase.trimmingTrailingSlash
    let url = "\(base)/x/web-interface/search/type"
    let v = try await biliGetJSON(ctx: ctx, url: url, query: [
        ("context", ""),
        ("search_type", "live"),
        ("cover_type", "user_cover"),
        ("order", ""),
        ("keyword", kw),
        ("category_id", ""),
        ("__refresh__", ""),
        ("_extra", ""),
        ("highlight", "0"),
        ("single_column", "0"),
        ("page", String(max(1, page))),
    ])
    let list = v.pointer("/data/result/live_room")?.asArray ?? []
    var items: [LiveRoomCard] = []
    for x in list {
        guard let rid = x.pointer("/roomid")?.asInt64.map(String.init), !rid.isEmpty else { continue }
        // 去除 <em> 高亮标签
        var title = x.pointer("/title")?.asString ?? ""
        if let regex = try? NSRegularExpression(pattern: "<.*?em.*?>", options: []) {
            title = regex.stringByReplacingMatches(in: title, range: NSRange(title.startIndex..., in: title), withTemplate: "")
        }
        var cover: String? = nil
        if let c = x.pointer("/cover")?.asString.flatMap(absCoverBili) {
            cover = c.contains("@") ? c : "\(c)@400w.jpg"
        }
        let userName = x.pointer("/uname")?.asString
        let online = x.pointer("/online")?.asInt64
        items.append(LiveRoomCard(site: .biliLive, roomId: rid, input: Site.biliLive.makeInput(roomId: rid), title: title, cover: cover, userName: userName, online: online.map(Int.init)))
    }
    return LiveRoomList(hasMore: items.count >= 40, items: items)
}

// MARK: - Douyu

private func douyuGetJSON(ctx: LiveDirectoryPlatformContext, url: String, query: [(String, String)] = []) async throws -> JSONValue {
    try await ctx.http.getJSON(url, query: query)
}

func douyuGetCategories(_ ctx: LiveDirectoryPlatformContext) async throws -> [LiveCategory] {
    let base = ctx.endpoints.douyuMBase.trimmingTrailingSlash
    let v = try await douyuGetJSON(ctx: ctx, url: "\(base)/api/cate/list")
    let cate1 = v.pointer("/data/cate1Info")?.asArray ?? []
    let cate2 = v.pointer("/data/cate2Info")?.asArray ?? []
    var out: [LiveCategory] = []
    for c1 in cate1 {
        let id = c1.pointer("/cate1Id")?.asInt64.map(String.init) ?? ""
        let name = c1.pointer("/cate1Name")?.asString ?? ""
        if id.isEmpty || name.isEmpty { continue }
        var subs: [LiveSubCategory] = []
        for c2 in cate2 {
            let pid = c2.pointer("/cate1Id")?.asInt64.map(String.init) ?? ""
            guard pid == id else { continue }
            let sid = c2.pointer("/cate2Id")?.asInt64.map(String.init) ?? ""
            let sname = c2.pointer("/cate2Name")?.asString ?? ""
            let pic = c2.pointer("/icon")?.asString
            if sid.isEmpty || sname.isEmpty { continue }
            subs.append(LiveSubCategory(id: sid, parentId: id, name: sname, pic: pic))
        }
        out.append(LiveCategory(id: id, name: name, children: subs))
    }
    out.sort { (Int($0.id) ?? 0) < (Int($1.id) ?? 0) }
    return out
}

private func douyuBuildCards(from rl: [JSONValue]) -> [LiveRoomCard] {
    var items: [LiveRoomCard] = []
    for x in rl {
        if (x.pointer("/type")?.asInt64 ?? 1) != 1 { continue }
        guard let rid = x.pointer("/rid")?.asInt64.map(String.init), !rid.isEmpty else { continue }
        let title = x.pointer("/rn")?.asString ?? ""
        let cover = x.pointer("/rs16")?.asString
        let userName = x.pointer("/nn")?.asString
        let online = x.pointer("/ol")?.asInt64
        items.append(LiveRoomCard(site: .douyu, roomId: rid, input: Site.douyu.makeInput(roomId: rid), title: title, cover: cover, userName: userName, online: online.map(Int.init)))
    }
    return items
}

func douyuGetRecommendRooms(_ ctx: LiveDirectoryPlatformContext, page: Int) async throws -> LiveRoomList {
    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let v = try await douyuGetJSON(ctx: ctx, url: "\(base)/japi/weblist/apinc/allpage/6/\(max(1, page))")
    let rl = v.pointer("/data/rl")?.asArray ?? []
    let items = douyuBuildCards(from: rl)
    let pgcnt = asI64(v.pointer("/data/pgcnt"))
    let hasMore: Bool
    if pgcnt > 0 {
        hasMore = Int64(max(1, page)) < pgcnt
    } else {
        hasMore = !items.isEmpty
    }
    return LiveRoomList(hasMore: hasMore, items: items)
}

func douyuGetCategoryRooms(_ ctx: LiveDirectoryPlatformContext, categoryId: String, page: Int) async throws -> LiveRoomList {
    let cid = categoryId.trimmingCharacters(in: .whitespaces)
    guard !cid.isEmpty else { throw LiveKitError.invalidInput("category_id is empty") }
    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let v = try await douyuGetJSON(ctx: ctx, url: "\(base)/gapi/rkc/directory/mixList/2_\(cid)/\(max(1, page))")
    let rl = v.pointer("/data/rl")?.asArray ?? []
    let items = douyuBuildCards(from: rl)
    let pgcnt = asI64(v.pointer("/data/pgcnt"))
    let hasMore: Bool
    if pgcnt > 0 {
        hasMore = Int64(max(1, page)) < pgcnt
    } else {
        hasMore = !items.isEmpty
    }
    return LiveRoomList(hasMore: hasMore, items: items)
}

func douyuSearchRooms(_ ctx: LiveDirectoryPlatformContext, keyword: String, page: Int) async throws -> LiveRoomList {
    let kw = keyword.trimmingCharacters(in: .whitespaces)
    guard !kw.isEmpty else { throw LiveKitError.invalidInput("keyword is empty") }
    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let url = "\(base)/japi/search/api/searchShow"
    let v = try await ctx.http.getJSON(url, query: [
        ("kw", kw),
        ("page", String(max(1, page))),
        ("pageSize", "20"),
    ], headers: [
        "Referer": "https://www.douyu.com/search/",
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    ])
    let itemsRaw = v.pointer("/data/relateShow")?.asArray ?? []
    var items: [LiveRoomCard] = []
    for x in itemsRaw {
        guard let rid = x.pointer("/rid")?.asInt64.map(String.init), !rid.isEmpty else { continue }
        let title = x.pointer("/roomName")?.asString ?? ""
        let cover = x.pointer("/roomSrc")?.asString
        let userName = x.pointer("/nickName")?.asString
        let hot = x.pointer("/hot")?.asString ?? ""
        let online = parseHotNum(hot)
        items.append(LiveRoomCard(site: .douyu, roomId: rid, input: Site.douyu.makeInput(roomId: rid), title: title, cover: cover, userName: userName, online: Int(online)))
    }
    return LiveRoomList(hasMore: !items.isEmpty, items: items)
}

// MARK: - Huya

private let HUYA_UA = "HYSDK(Windows, 30000002)_APP(pc_exe&7060000&official)_SDK(trans&2.32.3.5646)"
private let HUYA_UA_FALLBACK = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
private let HUYA_MOBILE_BASE = "https://m.huya.com/"

private func huyaIsHtmlLike(_ s: String) -> Bool {
    let t = s.drop(while: { $0.isWhitespace })
    return t.hasPrefix("<!DOCTYPE") || t.hasPrefix("<html") || t.hasPrefix("<")
}

private func huyaGetValue(ctx: LiveDirectoryPlatformContext, url: String, query: [(String, String)]) async throws -> JSONValue {
    do {
        return try await huyaDoRequest(ctx: ctx, url: url, query: query, ua: HUYA_UA)
    } catch {
        // 失败再用浏览器 UA 重试一次（对齐 Rust：保留原始错误）。
        return try await huyaDoRequest(ctx: ctx, url: url, query: query, ua: HUYA_UA_FALLBACK)
    }
}

private func huyaDoRequest(ctx: LiveDirectoryPlatformContext, url: String, query: [(String, String)], ua: String) async throws -> JSONValue {
    let text = try await ctx.http.getText(url, query: query, headers: [
        "User-Agent": ua,
        "Referer": HUYA_MOBILE_BASE,
        "Origin": HUYA_MOBILE_BASE,
        "Accept": "application/json, text/plain, */*",
    ])
    var body = text
    // 去 UTF-8 BOM。
    if body.hasPrefix("\u{FEFF}") { body.removeFirst() }
    let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty { throw LiveKitError.parse("huya response is empty") }
    if huyaIsHtmlLike(trimmed) {
        let snippet = String(trimmed.prefix(200))
        throw LiveKitError.parse("huya response is not json: \(snippet)")
    }
    do {
        return try JSONValue(parsing: body)
    } catch {
        let snippet = String(body.prefix(200))
        throw LiveKitError.parse("huya json parse failed: \(error); body=\(snippet)")
    }
}

private func huyaParseGid(_ v: JSONValue?) -> String? {
    guard let v else { return nil }
    if case .object(let map) = v, let val = map["value"]?.asString {
        return val.split(separator: ",").first.map(String.init)?.trimmingCharacters(in: .whitespaces)
    }
    if let n = v.asInt64 { return String(n) }
    if case .string(let s) = v { return s }
    return nil
}

func huyaGetCategories(_ ctx: LiveDirectoryPlatformContext) async throws -> [LiveCategory] {
    let base = ctx.endpoints.huyaLiveCdnBase.trimmingTrailingSlash
    let url = "\(base)/liveconfig/game/bussLive"
    var cats: [LiveCategory] = [
        LiveCategory(id: "1", name: "网游", children: []),
        LiveCategory(id: "2", name: "单机", children: []),
        LiveCategory(id: "8", name: "娱乐", children: []),
        LiveCategory(id: "3", name: "手游", children: []),
    ]
    var okAny = false
    var lastErr: Error?
    for i in 0..<cats.count {
        do {
            let v = try await huyaGetValue(ctx: ctx, url: url, query: [("bussType", cats[i].id)])
            okAny = true
            let arr = v.pointer("/data")?.asArray ?? []
            var subs: [LiveSubCategory] = []
            for x in arr {
                let gid = huyaParseGid(x.pointer("/gid")) ?? ""
                let name = x.pointer("/gameFullName")?.asString ?? ""
                if gid.isEmpty || name.isEmpty { continue }
                subs.append(LiveSubCategory(id: gid, parentId: cats[i].id, name: name, pic: "https://huyaimg.msstatic.com/cdnimage/game/\(gid)-MS.jpg"))
            }
            cats[i].children.append(contentsOf: subs)
        } catch {
            lastErr = error
            continue
        }
    }
    if !okAny {
        throw lastErr ?? LiveKitError.parse("huya categories failed")
    }
    return cats
}

private func huyaBuildCard(from x: JSONValue) -> LiveRoomCard? {
    func ridValue() -> String? {
        if let n = x.pointer("/profileRoom")?.asInt64 { return String(n) }
        return x.pointer("/profileRoom")?.asString
    }
    guard let rid = ridValue(), !rid.isEmpty else { return nil }
    var cover = x.pointer("/screenshot")?.asString ?? ""
    if !cover.isEmpty && !cover.contains("?") {
        cover += "?x-oss-process=style/w338_h190&"
    }
    let intro = x.pointer("/introduction")?.asString ?? ""
    let roomName = x.pointer("/roomName")?.asString ?? ""
    let title = intro.trimmingCharacters(in: .whitespaces).isEmpty ? roomName : intro
    let userName = x.pointer("/nick")?.asString
    func onlineValue() -> Int64? {
        if let s = x.pointer("/totalCount")?.asString, let n = Int64(s) { return n }
        return x.pointer("/totalCount")?.asInt64
    }
    let finalCover = cover.trimmingCharacters(in: .whitespaces).isEmpty ? nil : cover
    return LiveRoomCard(site: .huya, roomId: rid, input: Site.huya.makeInput(roomId: rid), title: title, cover: finalCover, userName: userName, online: onlineValue().map(Int.init))
}

private func huyaHasMoreFromPages(_ v: JSONValue, itemsEmpty: Bool) -> Bool {
    let page = v.pointer("/data/page")?.asInt64
    let totalPage = v.pointer("/data/totalPage")?.asInt64
    if let p = page, let t = totalPage { return p < t }
    return !itemsEmpty
}

func huyaGetRecommendRooms(_ ctx: LiveDirectoryPlatformContext, page: Int) async throws -> LiveRoomList {
    let base = ctx.endpoints.huyaBase.trimmingTrailingSlash
    let v = try await huyaGetValue(ctx: ctx, url: "\(base)/cache.php", query: [
        ("m", "LiveList"),
        ("do", "getLiveListByPage"),
        ("tagAll", "0"),
        ("page", String(max(1, page))),
    ])
    let datas = v.pointer("/data/datas")?.asArray ?? []
    let items = datas.compactMap(huyaBuildCard)
    return LiveRoomList(hasMore: huyaHasMoreFromPages(v, itemsEmpty: items.isEmpty), items: items)
}

func huyaGetCategoryRooms(_ ctx: LiveDirectoryPlatformContext, categoryId: String, page: Int) async throws -> LiveRoomList {
    let cid = categoryId.trimmingCharacters(in: .whitespaces)
    guard !cid.isEmpty else { throw LiveKitError.invalidInput("category_id is empty") }
    let base = ctx.endpoints.huyaBase.trimmingTrailingSlash
    let v = try await huyaGetValue(ctx: ctx, url: "\(base)/cache.php", query: [
        ("m", "LiveList"),
        ("do", "getLiveListByPage"),
        ("tagAll", "0"),
        ("gameId", cid),
        ("page", String(max(1, page))),
    ])
    let datas = v.pointer("/data/datas")?.asArray ?? []
    let items = datas.compactMap(huyaBuildCard)
    return LiveRoomList(hasMore: huyaHasMoreFromPages(v, itemsEmpty: items.isEmpty), items: items)
}

func huyaSearchRooms(_ ctx: LiveDirectoryPlatformContext, keyword: String, page: Int) async throws -> LiveRoomList {
    let kw = keyword.trimmingCharacters(in: .whitespaces)
    guard !kw.isEmpty else { throw LiveKitError.invalidInput("keyword is empty") }
    let base = ctx.endpoints.huyaSearchBase.trimmingTrailingSlash
    let start = (max(1, page) - 1) * 20
    let v = try await huyaGetValue(ctx: ctx, url: "\(base)/", query: [
        ("m", "Search"),
        ("do", "getSearchContent"),
        ("q", kw),
        ("uid", "0"),
        ("v", "4"),
        ("typ", "-5"),
        ("livestate", "0"),
        ("rows", "20"),
        ("start", String(start)),
    ])
    let list = v.pointer("/response/3/docs")?.asArray ?? []
    let numFound = v.pointer("/response/3/numFound")?.asInt64 ?? Int64(list.count)
    var items: [LiveRoomCard] = []
    for x in list {
        let rid = x.pointer("/room_id")?.asString ?? ""
        if rid.isEmpty { continue }
        var title = x.pointer("/game_introduction")?.asString ?? ""
        if title.trimmingCharacters(in: .whitespaces).isEmpty {
            title = x.pointer("/game_roomName")?.asString ?? ""
        }
        var cover = x.pointer("/game_screenshot")?.asString ?? ""
        if !cover.isEmpty && !cover.contains("?") {
            cover += "?x-oss-process=style/w338_h190&"
        }
        let userName = x.pointer("/game_nick")?.asString
        let online = x.pointer("/game_total_count")?.asInt64
        let finalCover = cover.trimmingCharacters(in: .whitespaces).isEmpty ? nil : cover
        items.append(LiveRoomCard(site: .huya, roomId: rid, input: Site.huya.makeInput(roomId: rid), title: title, cover: finalCover, userName: userName, online: online.map(Int.init)))
    }
    let hasMore = numFound > Int64(max(1, page)) * 20
    return LiveRoomList(hasMore: hasMore, items: items)
}

// MARK: - 字符串辅助

extension String {
    /// 去掉结尾的 `/`（对齐 Rust `trim_end_matches('/')`）。
    var trimmingTrailingSlash: String {
        var s = self
        while s.hasSuffix("/") { s.removeLast() }
        return s
    }
}
