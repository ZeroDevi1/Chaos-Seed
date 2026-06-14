import Foundation

/// BiliLive 直播源解析。忠实移植 Rust `chaos_core::livestream::platforms::bili_live`。
///
/// 复杂点：
/// - buvid cookie 缓存（移植 `buvid_cookie_cache`，用 OnceLock 风格的全局）。
/// - v2 getRoomPlayInfo 的 codec 选择（`pick_best_codec`）。
/// - 多档清晰度枚举（accept_qn + g_qn_desc）+ 最佳可播放 URL 绑定。
/// - resolve 路径：v2 → v1 playUrl → html fallback，最后兜底 v2 URL。
private let BILI_LS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0"
private let BILI_LS_REFERER = "https://live.bilibili.com/"
private let BILI_LS_ORIGIN = "https://live.bilibili.com"

/// buvid cookie 缓存：None=未尝试；Some("")=失败不再重试；Some(cookie)=可用。
/// 对齐 Rust `buvid_cookie_cache()`。
private final class BuvidCookieCache: @unchecked Sendable {
    static let shared = BuvidCookieCache()
    private let lock = NSLock()
    private var cached: String?? = nil // 外层 Optional 表示"是否尝试过"

    func get() -> String?? {
        lock.lock(); defer { lock.unlock() }
        return cached
    }
    func set(_ value: String?) {
        lock.lock(); defer { lock.unlock() }
        cached = value
    }
}

private func biliEnsureBuvidCookie(ctx: LivestreamContext) async -> String? {
    if let tried = BuvidCookieCache.shared.get() {
        // tried = nil 表示缓存为空串（失败）→ 返回 nil；否则返回缓存。
        return tried?.isEmpty == true ? nil : tried
    }
    let url = "https://api.bilibili.com/x/frontend/finger/spi"
    guard let json = try? await ctx.http.getJSON(url, headers: [
        "User-Agent": BILI_LS_UA,
        "Referer": BILI_LS_REFERER,
        "Origin": BILI_LS_ORIGIN,
    ]) else {
        BuvidCookieCache.shared.set("")
        return nil
    }
    if json.pointer("/code")?.asInt64 ?? -1 != 0 {
        BuvidCookieCache.shared.set("")
        return nil
    }
    let b3 = json.pointer("/data/b_3")?.asString?.trimmingCharacters(in: .whitespaces) ?? ""
    let b4 = json.pointer("/data/b_4")?.asString?.trimmingCharacters(in: .whitespaces) ?? ""
    if b3.isEmpty || b4.isEmpty {
        BuvidCookieCache.shared.set("")
        return nil
    }
    let cookie = "buvid3=\(b3); buvid4=\(b4);"
    BuvidCookieCache.shared.set(cookie)
    return cookie
}

private func biliHeaders(cookie: String?) -> [String: String] {
    var h: [String: String] = [
        "User-Agent": BILI_LS_UA,
        "Referer": BILI_LS_REFERER,
        "Origin": BILI_LS_ORIGIN,
    ]
    if let cookie { h["Cookie"] = cookie }
    return h
}

private func biliGetJSON(ctx: LivestreamContext, url: String) async throws -> JSONValue {
    let cookie = await biliEnsureBuvidCookie(ctx: ctx)
    return try await ctx.http.getJSON(url, headers: biliHeaders(cookie: cookie))
}

private func biliGetText(ctx: LivestreamContext, url: String) async throws -> String {
    let cookie = await biliEnsureBuvidCookie(ctx: ctx)
    return try await ctx.http.getText(url, headers: biliHeaders(cookie: cookie))
}

// MARK: - codec 选择（对齐 pick_best_codec）

private struct CodecKey: Comparable {
    let protocolRank: UInt8
    let formatRank: UInt8
    let maxAccept: Int
    let codecRank: UInt8
    static func < (lhs: CodecKey, rhs: CodecKey) -> Bool {
        if lhs.protocolRank != rhs.protocolRank { return lhs.protocolRank < rhs.protocolRank }
        if lhs.formatRank != rhs.formatRank { return lhs.formatRank < rhs.formatRank }
        if lhs.maxAccept != rhs.maxAccept { return lhs.maxAccept > rhs.maxAccept }
        return lhs.codecRank < rhs.codecRank
    }
}

private func codecAcceptQnMax(_ codec: JSONValue) -> Int {
    codec.pointer("/accept_qn")?.asArray?.compactMap { $0.asInt64 }.map(Int.init).max() ?? -1
}

/// 在 streams 树里选最佳 codec 分支。
/// 排序：http_stream > http_hls > others；flv/fmp4 > 其他；accept_qn 高优先；avc 优先。
private func pickBestCodec(_ streams: [JSONValue]) -> JSONValue? {
    var best: (JSONValue, CodecKey)?
    for s in streams {
        let proto = s.pointer("/protocol_name")?.asString ?? ""
        let protoRank: UInt8
        switch proto {
        case "http_stream": protoRank = 0
        case "http_hls": protoRank = 1
        default: protoRank = 2
        }
        guard let formats = s.pointer("/format")?.asArray else { continue }
        for f in formats {
            let fmt = f.pointer("/format_name")?.asString ?? ""
            let fmtRank: UInt8 = (fmt == "flv" || fmt == "fmp4") ? 0 : 1
            guard let codecs = f.pointer("/codec")?.asArray else { continue }
            for c in codecs {
                let base = (c.pointer("/base_url")?.asString ?? "").trimmingCharacters(in: .whitespaces)
                let urlInfoLen = c.pointer("/url_info")?.asArray?.count ?? 0
                if base.isEmpty || urlInfoLen == 0 { continue }
                let maxAccept = codecAcceptQnMax(c)
                let codecName = c.pointer("/codec_name")?.asString ?? ""
                let codecRank: UInt8 = codecName == "avc" ? 0 : 1
                let key = CodecKey(protocolRank: protoRank, formatRank: fmtRank, maxAccept: maxAccept, codecRank: codecRank)
                if best == nil || key > best!.1 {
                    best = (c, key)
                }
            }
        }
    }
    return best?.0
}

/// 对齐 `parse_room_playinfo_value`：枚举 accept_qn + g_qn_desc，按 requested_qn 绑定 URL。
private func biliParseRoomPlayInfoValue(_ v: JSONValue, requestedQn: Int?) throws -> [StreamVariant] {
    if getBool(v, "/data/encrypted") ?? false && !(getBool(v, "/data/pwd_verified") ?? true) {
        throw LiveKitError.needPassword
    }
    guard let qnDesc = v.pointer("/data/playurl_info/playurl/g_qn_desc")?.asArray else {
        throw LiveKitError.parse("missing g_qn_desc")
    }
    guard let streams = v.pointer("/data/playurl_info/playurl/stream")?.asArray else {
        throw LiveKitError.parse("missing stream")
    }
    guard let codec = pickBestCodec(streams) else {
        throw LiveKitError.parse("no suitable codec")
    }
    guard let currentQn = codec.pointer("/current_qn")?.asInt64 else {
        throw LiveKitError.parse("missing current_qn")
    }
    let currentQnI = Int(currentQn)
    let acceptQn: [Int] = (codec.pointer("/accept_qn")?.asArray ?? []).compactMap { $0.asInt64.map(Int.init) }
    guard let baseUrl = codec.pointer("/base_url")?.asString else {
        throw LiveKitError.parse("missing base_url")
    }
    guard let urlInfo = codec.pointer("/url_info")?.asArray else {
        throw LiveKitError.parse("missing url_info")
    }
    var urls: [String] = urlInfo.compactMap { ui in
        guard let host = ui.pointer("/host")?.asString else { return nil }
        let extra = ui.pointer("/extra")?.asString ?? ""
        return "\(host)\(baseUrl)\(extra)"
    }
    urls = Mbga.sortUrls(urls)

    var out: [StreamVariant] = []
    for item in qnDesc {
        let qn = Int(item.pointer("/qn")?.asInt64 ?? -1)
        let label = item.pointer("/desc")?.asString ?? ""
        if qn <= 0 || label.isEmpty { continue }
        if !acceptQn.contains(qn) { continue }
        var variant = StreamVariant(id: biliMakeVariantId(qn: qn, label: label), label: label, quality: qn, rate: nil, url: nil, backupUrls: [])
        let shouldBind: Bool
        if let r = requestedQn {
            shouldBind = (r == qn)
        } else {
            shouldBind = (qn == currentQnI)
        }
        if shouldBind && !urls.isEmpty {
            variant.url = urls[0]
            variant.backupUrls = Array(urls.dropFirst())
        }
        out.append(variant)
    }
    return out
}

/// 对齐 `extract_v2_codec_current_qn_and_urls`。
private func biliExtractV2CurrentQnAndUrls(_ v: JSONValue) throws -> (Int, [String]) {
    guard let streams = v.pointer("/data/playurl_info/playurl/stream")?.asArray else {
        throw LiveKitError.parse("missing stream")
    }
    guard let codec = pickBestCodec(streams) else {
        throw LiveKitError.parse("no suitable codec")
    }
    guard let currentQn = codec.pointer("/current_qn")?.asInt64 else {
        throw LiveKitError.parse("missing current_qn")
    }
    guard let baseUrl = codec.pointer("/base_url")?.asString else {
        throw LiveKitError.parse("missing base_url")
    }
    guard let urlInfo = codec.pointer("/url_info")?.asArray else {
        throw LiveKitError.parse("missing url_info")
    }
    var urls: [String] = urlInfo.compactMap { ui in
        guard let host = ui.pointer("/host")?.asString else { return nil }
        let extra = ui.pointer("/extra")?.asString ?? ""
        return "\(host)\(baseUrl)\(extra)"
    }
    urls = Mbga.sortUrls(urls)
    return (Int(currentQn), urls)
}

private struct RoomPlayInfoV2 {
    let vars: [StreamVariant]
    let currentQn: Int
    let urls: [String]
}

private func biliFetchRoomPlayInfo(ctx: LivestreamContext, rid: Int64, qn: Int) async throws -> RoomPlayInfoV2 {
    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let url: String
    if qn > 0 {
        url = "\(base)/xlive/web-room/v2/index/getRoomPlayInfo?room_id=\(rid)&protocol=0,1&format=0,1,2&codec=0,1&qn=\(qn)&platform=web&ptype=8&dolby=5"
    } else {
        url = "\(base)/xlive/web-room/v2/index/getRoomPlayInfo?room_id=\(rid)&protocol=0,1&format=0,1,2&codec=0,1&platform=web&ptype=8&dolby=5"
    }
    let json = try await biliGetJSON(ctx: ctx, url: url)
    let (currentQn, urls) = try biliExtractV2CurrentQnAndUrls(json)
    let vars_ = try biliParseRoomPlayInfoValue(json, requestedQn: qn > 0 ? qn : nil)
    return RoomPlayInfoV2(vars: vars_, currentQn: currentQn, urls: urls)
}

private func biliFetchRoomPlayInfoList(ctx: LivestreamContext, rid: Int64) async throws -> [StreamVariant] {
    // 注意：枚举清晰度时不传 qn（对齐 dart_simple_live）。
    try await biliFetchRoomPlayInfo(ctx: ctx, rid: rid, qn: 0).vars
}

/// 对齐 `fetch_playinfo_list`：NeedPassword 上抛；其它失败 → v1 playUrl → html fallback。
private func biliFetchPlayInfoList(ctx: LivestreamContext, rid: Int64) async throws -> [StreamVariant] {
    do {
        return try await biliFetchRoomPlayInfoList(ctx: ctx, rid: rid)
    } catch LiveKitError.needPassword {
        throw LiveKitError.needPassword
    } catch {
        if let vars = try? await biliFetchPlayUrl(ctx: ctx, rid: rid, qn: 0) {
            return vars
        }
        return try await biliFetchHtmlFallback(ctx: ctx, rid: rid, qn: 0)
    }
}

private func biliFetchPlayUrl(ctx: LivestreamContext, rid: Int64, qn: Int) async throws -> [StreamVariant] {
    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let url = "\(base)/room/v1/Room/playUrl?cid=\(rid)&qn=\(qn)&platform=web"
    let json = try await biliGetJSON(ctx: ctx, url: url)
    guard let currentQn = getI64(json, "/data/current_qn") else {
        throw LiveKitError.parse("missing data.current_qn")
    }
    guard let qnDesc = json.pointer("/data/quality_description")?.asArray else {
        throw LiveKitError.parse("missing data.quality_description")
    }
    var urls: [String] = []
    if let durl = json.pointer("/data/durl")?.asArray {
        urls = durl.compactMap { $0.pointer("/url")?.asString }
    }
    urls = Mbga.sortUrls(urls)

    var out: [StreamVariant] = []
    for item in qnDesc {
        let descQn = Int(item.pointer("/qn")?.asInt64 ?? -1)
        let label = item.pointer("/desc")?.asString ?? ""
        if descQn <= 0 || label.isEmpty { continue }
        var v = StreamVariant(id: biliMakeVariantId(qn: descQn, label: label), label: label, quality: descQn, rate: nil, url: nil, backupUrls: [])
        if qn > 0 {
            if descQn == qn && !urls.isEmpty {
                v.url = urls[0]
                v.backupUrls = Array(urls.dropFirst())
            }
        } else if descQn == Int(currentQn) && !urls.isEmpty {
            v.url = urls[0]
            v.backupUrls = Array(urls.dropFirst())
        }
        out.append(v)
    }
    return out
}

private func pickVariantWithUrl(_ vars: [StreamVariant], qn: Int) -> StreamVariant? {
    vars.first { $0.quality == qn && ($0.url?.trimmingCharacters(in: .whitespaces).isEmpty == false) }
}

/// 对齐 `resolve_variant_for_qn`：v2 → v1 → html，最后兜底 v2 URL。
private func biliResolveVariantForQn(ctx: LivestreamContext, rid: Int64, qn: Int) async throws -> StreamVariant? {
    var v2LastResort: ([StreamVariant], [String])?
    do {
        let info = try await biliFetchRoomPlayInfo(ctx: ctx, rid: rid, qn: qn)
        if let v = pickVariantWithUrl(info.vars, qn: qn), info.currentQn == qn {
            return v
        }
        if !info.urls.isEmpty {
            v2LastResort = (info.vars, info.urls)
        }
    } catch LiveKitError.needPassword {
        throw LiveKitError.needPassword
    } catch {}

    if let vars = try? await biliFetchPlayUrl(ctx: ctx, rid: rid, qn: qn),
       let v = pickVariantWithUrl(vars, qn: qn) {
        return v
    }

    do {
        let vars = try await biliFetchHtmlFallback(ctx: ctx, rid: rid, qn: qn)
        if let v = pickVariantWithUrl(vars, qn: qn) {
            return v
        }
    } catch LiveKitError.needPassword {
        throw LiveKitError.needPassword
    } catch {}

    if let (vars, urls) = v2LastResort, var v = vars.first(where: { $0.quality == qn }) {
        v.url = urls[0]
        v.backupUrls = Array(urls.dropFirst())
        return v
    }
    return nil
}

private func biliFetchHtmlFallback(ctx: LivestreamContext, rid: Int64, qn: Int) async throws -> [StreamVariant] {
    let base = ctx.endpoints.biliLiveBase.trimmingTrailingSlash
    let url = "\(base)/\(rid)"
    let text = try await biliGetText(ctx: ctx, url: url)
    let marker = "<script>window.__NEPTUNE_IS_MY_WAIFU__="
    guard let blob = text
        .components(separatedBy: marker)
        .dropFirst()
        .first?
        .components(separatedBy: "</script>")
        .first?
        .trimmingCharacters(in: .whitespacesAndNewlines) else {
        throw LiveKitError.parse("missing __NEPTUNE_IS_MY_WAIFU__")
    }
    let json = try JSONValue(parsing: blob)
    guard let roomInit = json.pointer("/roomInitRes") else {
        throw LiveKitError.parse("missing roomInitRes")
    }
    var vars = try biliParseRoomPlayInfoValue(roomInit, requestedQn: qn > 0 ? qn : nil)
    // 统一 variant id 格式。
    for i in vars.indices {
        vars[i].id = biliMakeVariantId(qn: vars[i].quality, label: vars[i].label)
    }
    return vars
}

private func applyDropInaccessible(_ vars: [StreamVariant], options: ResolveOptions) -> [StreamVariant] {
    guard options.dropInaccessibleHighQualities else { return vars }
    var result = vars
    let resolvedQ = result.filter { $0.url?.isEmpty == false }.map(\.quality).max()
    if let q = resolvedQ {
        result.removeAll { $0.quality > q }
    }
    return result
}

// MARK: - decode_manifest / resolve_variant

func biliDecodeManifest(ctx: LivestreamContext, roomId: String, rawInput: String, options: ResolveOptions) async throws -> LiveManifest {
    let ridStr = roomId.trimmingCharacters(in: .whitespaces)
    guard !ridStr.isEmpty else { throw LiveKitError.invalidInput("empty room id") }

    let base = ctx.endpoints.biliLiveApiBase.trimmingTrailingSlash
    let infoURL = "\(base)/room/v1/Room/get_info?room_id=\(ridStr)"
    let json = try await biliGetJSON(ctx: ctx, url: infoURL)
    guard let rid = getI64(json, "/data/room_id") else {
        throw LiveKitError.parse("missing data.room_id")
    }
    let title = getStr(json, "/data/title") ?? ""
    let isLiving = (getI64(json, "/data/live_status") ?? 0) == 1
    let cover = getStr(json, "/data/user_cover")?.filter { !$0.isWhitespace }
    Log.network.debug("bili get_info: input_rid=\(ridStr) canonical_rid=\(rid) living=\(isLiving) title=\(title)")

    var name: String? = nil
    var avatar: String? = nil
    let anchorURL = "\(base)/live_user/v1/UserInfo/get_anchor_in_room?roomid=\(rid)"
    if let anchor = try? await biliGetJSON(ctx: ctx, url: anchorURL) {
        name = getStr(anchor, "/data/info/uname")?.filter { !$0.isWhitespace }
        avatar = getStr(anchor, "/data/info/face")?.filter { !$0.isWhitespace }
    } else {
        Log.network.debug("bili anchor: 不可用（get_anchor_in_room 失败），继续无主播信息")
    }
    let info = LiveInfo(title: title, name: name, avatar: avatar, cover: cover, isLiving: isLiving)

    var vars = try await biliFetchPlayInfoList(ctx: ctx, rid: rid)
    Log.network.debug("bili playinfo: 枚举到 \(vars.count) 个清晰度 qns=\(vars.map(\.quality))")
    // 取最高 qn 尝试绑定 URL（对齐 Rust 行为，避免只剩低清）。
    var qns = Set(vars.map(\.quality)).sorted(by: >)
    for qn in qns.prefix(8) {
        let already = vars.contains { $0.quality == qn && $0.url?.isEmpty == false }
        if already { break }
        // biliResolveVariantForQn throws & returns Optional → try? gives StreamVariant??
        if let rv = (try? await biliResolveVariantForQn(ctx: ctx, rid: rid, qn: qn)) ?? nil {
            Log.network.debug("bili resolve: qn=\(qn) 绑定到 url=\(rv.url != nil) backups=\(rv.backupUrls.count)")
            if let idx = vars.firstIndex(where: { $0.quality == qn }) {
                vars[idx].url = rv.url
                vars[idx].backupUrls = rv.backupUrls
            }
            break
        } else {
            Log.network.debug("bili resolve: qn=\(qn) 未能获取可播放 URL")
        }
    }
    qns.removeAll()

    vars = applyDropInaccessible(vars, options: options)
    vars.sort { $0.quality > $1.quality }
    Log.network.debug("bili manifest 完成: variants=\(vars.map { "\($0.label)(qn=\($0.quality),url=\($0.url != nil))" })")

    return LiveManifest(
        site: .biliLive,
        roomId: String(rid),
        rawInput: rawInput,
        info: info,
        playback: PlaybackHints(referer: "https://live.bilibili.com/", userAgent: nil),
        variants: vars
    )
}

func biliResolveVariant(ctx: LivestreamContext, roomId: String, variantId: String) async throws -> StreamVariant {
    guard let rid = Int64(roomId.trimmingCharacters(in: .whitespaces)) else {
        throw LiveKitError.invalidInput("invalid room_id")
    }
    let vid = variantId.trimmingCharacters(in: .whitespaces)
    guard let firstColon = vid.firstIndex(of: ":") else {
        throw LiveKitError.invalidInput("invalid variant_id")
    }
    let rest = vid[vid.index(after: firstColon)...]
    guard let secondColon = rest.firstIndex(of: ":") else {
        throw LiveKitError.invalidInput("invalid variant_id")
    }
    let qnStr = rest[rest.startIndex..<secondColon]
    guard let qn = Int(qnStr) else {
        throw LiveKitError.invalidInput("invalid qn")
    }
    guard var v = try await biliResolveVariantForQn(ctx: ctx, rid: rid, qn: qn) else {
        throw LiveKitError.parse("requested quality not accessible")
    }
    v.id = biliMakeVariantId(qn: qn, label: v.label)
    return v
}
