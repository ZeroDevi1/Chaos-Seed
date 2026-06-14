import Foundation

// MARK: - Douyu 直播源解析（移植 `livestream/platforms/douyu`）

private func douyuGetText(_ ctx: LivestreamContext, url: String) async throws -> String {
    try await ctx.http.getText(url)
}
private func douyuGetJSON(_ ctx: LivestreamContext, url: String) async throws -> JSONValue {
    try await ctx.http.getJSON(url)
}
private func douyuPostFormJSON(_ ctx: LivestreamContext, url: String, form: [(String, String)]) async throws -> JSONValue {
    try await ctx.http.postFormJSON(url, form: form)
}

private func douyuUnescapeBackslashQuotes(_ s: String) -> String {
    // 对齐 Rust：执行 2 次 \" → "。
    var out = s
    for _ in 0..<2 {
        if out.contains("\\\"") {
            out = out.replacingOccurrences(of: "\\\"", with: "\"")
        }
    }
    return out
}

private func douyuParseRoomIdFromHTML(_ html: String) throws -> (Int64, Bool) {
    // 优先从 roomInfo 的 JSON 对象提取。
    let markers = ["\\\"roomInfo\\\"", "\"roomInfo\"", "roomInfo"]
    for marker in markers {
        if let obj = BraceExtract.extractBalancedObject(afterMarker: marker, in: html) {
            let unescaped = douyuUnescapeBackslashQuotes(obj)
            if let json = try? JSONValue(parsing: unescaped),
               let rid = getI64(json, "/room/room_id") {
                let isLiving = (getI64(json, "/room/show_status") ?? 0) == 1
                return (rid, isLiving)
            }
        }
    }
    // Fallback：正则扫描。
    guard let ridRegex = try? NSRegularExpression(pattern: #"room_id\\?"?\s*[:=]\s*"?(\d+)"?"#, options: []),
          let ridMatch = ridRegex.firstMatch(in: html, range: NSRange(html.startIndex..., in: html)),
          let ridRange = Range(ridMatch.range(at: 1), in: html),
          let rid = Int64(html[ridRange]) else {
        throw LiveKitError.parse("douyu: missing room_id")
    }
    let showRegex = try? NSRegularExpression(pattern: #"show_status\\?"?\s*[:=]\s*"?(\d+)"?"#, options: [])
    let isLiving: Bool
    if let showRegex, let m = showRegex.firstMatch(in: html, range: NSRange(html.startIndex..., in: html)),
       let r = Range(m.range(at: 1), in: html), let n = Int64(html[r]) {
        isLiving = n == 1
    } else {
        isLiving = false
    }
    return (rid, isLiving)
}

private func douyuParseBetardInfo(_ json: JSONValue) -> LiveInfo {
    let title = getStr(json, "/room/room_name") ?? ""
    let name = getStr(json, "/room/nickname")?.filter { !$0.isWhitespace }
    let avatar = getStr(json, "/room/avatar/big")?.filter { !$0.isWhitespace }
    let cover = getStr(json, "/room/room_pic")?.filter { !$0.isWhitespace }
    var isLiving = false
    if let v = json.pointer("/room/show_status") {
        if let n = v.asInt64 { isLiving = n == 1 }
        else if let s = v.asString { isLiving = s == "1" }
    }
    return LiveInfo(title: title, name: name, avatar: avatar, cover: cover, isLiving: isLiving)
}

private func douyuBuildCDNUrl(_ ctx: LivestreamContext, xp2pDomain: String, rtmpLive: String) -> String? {
    let prefix = rtmpLive.split(separator: ".").first.map(String.init)?.trimmingCharacters(in: .whitespaces) ?? ""
    if prefix.isEmpty || xp2pDomain.trimmingCharacters(in: .whitespaces).isEmpty { return nil }
    return "\(ctx.endpoints.douyuCdnScheme)://\(xp2pDomain.trimmingCharacters(in: .whitespaces))/\(prefix).xs"
}

private func douyuStableUuidLike(_ env: EnvConfig) -> String {
    String(format: "%016llx", env.nextUInt64())
}

/// 对齐 `build_play_urls`：返回 (flvUrl, p2pUrls)。
private func douyuBuildPlayUrls(
    ctx: LivestreamContext,
    env: EnvConfig,
    rtmpUrl: String,
    rtmpLive: String,
    p2pMeta: JSONValue?,
    cdnHosts: [String]
) -> (String, [String]) {
    let flvUrl = "\(rtmpUrl.trimmingTrailingSlash)/\(rtmpLive)"
    guard let meta = p2pMeta else { return (flvUrl, []) }
    let domain = meta.pointer("/xp2p_domain")?.asString?.trimmingCharacters(in: .whitespaces) ?? ""
    if domain.isEmpty { return (flvUrl, []) }
    let delay = meta.pointer("/xp2p_txDelay")?.asInt64 ?? 0
    let secret = meta.pointer("/xp2p_txSecret")?.asString ?? ""
    let time = meta.pointer("/xp2p_txTime")?.asString ?? ""

    let replaced = rtmpLive.replacingOccurrences(of: "flv", with: "xs")
    var parts = replaced.split(separator: "&").map(String.init)
    parts.append("delay=\(delay)")
    parts.append("txSecret=\(secret)")
    parts.append("txTime=\(time)")
    parts.append("uuid=\(douyuStableUuidLike(env))")
    let xsString = "\(domain)/live/\(parts.joined(separator: "&"))"

    var p2pUrls: [String] = []
    for h in cdnHosts where !h.trimmingCharacters(in: .whitespaces).isEmpty {
        p2pUrls.append("\(ctx.endpoints.douyuP2pScheme)://\(h)/\(xsString)")
    }
    return (flvUrl, p2pUrls)
}

private func douyuFetchEncryption(_ ctx: LivestreamContext, did: String) async throws -> DouyuEncryption {
    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let url = "\(base)/wgapi/livenc/liveweb/websec/getEncryption?did=\(did)"
    let json = try await douyuGetJSON(ctx, url: url)
    if (getI64(json, "/error") ?? -1) != 0 {
        throw LiveKitError.parse("douyu: encryption error")
    }
    guard let data = json.pointer("/data") else {
        throw LiveKitError.parse("douyu: missing data")
    }
    return DouyuEncryption(
        key: getStr(data, "/key") ?? "",
        randStr: getStr(data, "/rand_str") ?? "",
        encTime: Int(getI64(data, "/enc_time") ?? 0),
        encData: getStr(data, "/enc_data") ?? "",
        isSpecial: Int(getI64(data, "/is_special") ?? 0)
    )
}

/// 对齐 `fetch_h5_play`：返回 (variants, currentRate)。
private func douyuFetchH5Play(ctx: LivestreamContext, env: EnvConfig, rid: Int64, rate: Int) async throws -> ([StreamVariant], Int) {
    let did = env.douyuDid()
    let enc = try await douyuFetchEncryption(ctx, did: did)
    let ts = env.nowS()
    let auth = enc.auth(rid: String(rid), ts: ts)

    let form: [(String, String)] = [
        ("enc_data", enc.encData),
        ("tt", String(ts)),
        ("did", did),
        ("auth", auth),
        ("cdn", ""),
        ("rate", String(rate)),
        ("hevc", "0"),
        ("fa", "0"),
        ("ive", "0"),
    ]

    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let url = "\(base)/lapi/live/getH5PlayV1/\(rid)"
    let json = try await douyuPostFormJSON(ctx, url: url, form: form)
    guard let data = json.pointer("/data") else {
        throw LiveKitError.parse("douyu: missing data")
    }
    let currentRate = Int(getI64(data, "/rate") ?? 0)
    let rtmpUrl = getStr(data, "/rtmp_url") ?? ""
    let rtmpLive = getStr(data, "/rtmp_live") ?? ""
    guard let multirates = data.pointer("/multirates")?.asArray else {
        throw LiveKitError.parse("douyu: missing multirates")
    }

    var cdnHosts: [String] = []
    let p2pMeta = data.pointer("/p2pMeta")
    if let meta = p2pMeta, let domain = meta.pointer("/xp2p_domain")?.asString, !domain.isEmpty {
        if let cdnURL = douyuBuildCDNUrl(ctx, xp2pDomain: domain, rtmpLive: rtmpLive) {
            if let cdnJSON = try? await douyuGetJSON(ctx, url: cdnURL) {
                for k in ["sug", "bak"] {
                    if let arr = cdnJSON.pointer("/\(k)")?.asArray {
                        for it in arr {
                            if let s = it.asString { cdnHosts.append(s) }
                        }
                    }
                }
            }
        }
    }

    let (flvUrl, p2pUrls) = douyuBuildPlayUrls(ctx: ctx, env: env, rtmpUrl: rtmpUrl, rtmpLive: rtmpLive, p2pMeta: p2pMeta, cdnHosts: cdnHosts)
    var urls: [String] = [flvUrl]
    urls.append(contentsOf: p2pUrls)

    var variants: [StreamVariant] = []
    for mr in multirates {
        let label = mr.pointer("/name")?.asString ?? ""
        let mrRate = Int(mr.pointer("/rate")?.asInt64 ?? -1)
        let bit = Int(mr.pointer("/bit")?.asInt64 ?? -1)
        if label.isEmpty || mrRate < 0 || bit < 0 { continue }
        var v = StreamVariant(id: douyuMakeVariantId(rate: mrRate, label: label), label: label, quality: bit, rate: mrRate, url: nil, backupUrls: [])
        if mrRate == currentRate && !urls.isEmpty {
            v.url = urls[0]
            v.backupUrls = Array(urls.dropFirst())
        }
        variants.append(v)
    }
    variants.sort { $0.quality > $1.quality }
    return (variants, currentRate)
}

func douyuDecodeManifest(ctx: LivestreamContext, roomId: String, rawInput: String, options: ResolveOptions) async throws -> LiveManifest {
    let base = ctx.endpoints.douyuBase.trimmingTrailingSlash
    let url = "\(base)/\(roomId.trimmingCharacters(in: CharacterSet(charactersIn: "/")))"
    let html = try await douyuGetText(ctx, url: url)
    let (rid, isLiving) = try douyuParseRoomIdFromHTML(html)

    let betard = try await douyuGetJSON(ctx, url: "\(base)/betard/\(rid)")
    var info = douyuParseBetardInfo(betard)
    info.isLiving = isLiving

    let (variants, _) = try await douyuFetchH5Play(ctx: ctx, env: ctx.env, rid: rid, rate: 0)

    return LiveManifest(
        site: .douyu,
        roomId: String(rid),
        rawInput: rawInput,
        info: info,
        playback: PlaybackHints(referer: "https://www.douyu.com/", userAgent: nil),
        variants: variants
    )
}

func douyuResolveVariant(ctx: LivestreamContext, roomId: String, variantId: String) async throws -> StreamVariant {
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
    let rateStr = rest[rest.startIndex..<secondColon]
    guard let rate = Int(rateStr) else {
        throw LiveKitError.invalidInput("invalid rate")
    }
    let (vars, currentRate) = try await douyuFetchH5Play(ctx: ctx, env: ctx.env, rid: rid, rate: rate)
    guard var v = vars.first(where: { $0.rate == rate }) else {
        throw LiveKitError.parse("variant not found")
    }
    if v.url == nil && currentRate != rate {
        // 对齐 Rust：no-op
    }
    v.id = douyuMakeVariantId(rate: rate, label: v.label)
    return v
}

// MARK: - Huya 直播源解析（移植 `livestream/platforms/huya`）

private func huyaLsGetText(_ ctx: LivestreamContext, url: String, ua: String?) async throws -> String {
    var headers: [String: String] = [:]
    if let ua { headers["User-Agent"] = ua }
    return try await ctx.http.getText(url, headers: headers)
}

private func huyaLsGetJSON(_ ctx: LivestreamContext, url: String, ua: String?) async throws -> JSONValue {
    var headers: [String: String] = [:]
    if let ua { headers["User-Agent"] = ua }
    return try await ctx.http.getJSON(url, headers: headers)
}

private func huyaPickTitle(roomName: String, intro: String, fallback: String) -> String {
    if !roomName.trimmingCharacters(in: .whitespaces).isEmpty { return roomName }
    if !intro.trimmingCharacters(in: .whitespaces).isEmpty { return intro }
    return fallback
}

private func huyaExtractProfileRoomId(_ html: String) -> Int64? {
    guard let regex = try? NSRegularExpression(pattern: #"profileRoom["']?\s*[:=]\s*"?(\d+)"?"#, options: []) else { return nil }
    guard let m = regex.firstMatch(in: html, range: NSRange(html.startIndex..., in: html)),
          let r = Range(m.range(at: 1), in: html) else { return nil }
    return Int64(html[r])
}

private func huyaParseBitrateInfo(_ s: String) -> [(String, Int)] {
    let trimmed = s.trimmingCharacters(in: .whitespaces)
    if trimmed.isEmpty { return [] }
    guard let json = try? JSONValue(parsing: trimmed) else { return [] }
    guard let arr = json.asArray else { return [] }
    var out: [(String, Int)] = []
    for it in arr {
        let name = it.pointer("/sDisplayName")?.asString ?? ""
        let br = Int(it.pointer("/iBitRate")?.asInt64 ?? -1)
        if !name.isEmpty && br >= 0 { out.append((name, br)) }
    }
    return out
}

/// (streamName, presenterUid, flvUrl, suffix, antiCode)
private typealias HuyaStream = (streamName: String, presenterUid: UInt32, flvUrl: String, suffix: String, antiCode: String)

private func huyaParseStreamInfos(_ v: JSONValue) -> [HuyaStream] {
    var out: [HuyaStream] = []
    guard let arr = v.pointer("/data/stream/baseSteamInfoList")?.asArray else { return out }
    for it in arr {
        let sStreamName = it.pointer("/sStreamName")?.asString ?? ""
        var presenterUidI = it.pointer("/lPresenterUid")?.asInt64
        if presenterUidI == nil, let parsed = it.pointer("/lPresenterUid")?.asString.flatMap({ Int64($0) }) {
            presenterUidI = parsed
        }
        let presenterUid = max(0, min(Int64(UInt32.max), presenterUidI ?? 0))
        let sFlvUrl = it.pointer("/sFlvUrl")?.asString ?? ""
        let sSuffix = it.pointer("/sFlvUrlSuffix")?.asString ?? ""
        let sAnti = it.pointer("/sFlvAntiCode")?.asString ?? ""
        if !sStreamName.isEmpty && !sFlvUrl.isEmpty && !sSuffix.isEmpty && !sAnti.isEmpty && presenterUid > 0 {
            out.append((sStreamName, UInt32(presenterUid), sFlvUrl, sSuffix, sAnti))
        }
    }
    return out
}

func huyaDecodeManifest(ctx: LivestreamContext, roomId: String, rawInput: String, options: ResolveOptions) async throws -> LiveManifest {
    let rid: Int64
    if let n = Int64(roomId.trimmingCharacters(in: .whitespaces)) {
        rid = n
    } else {
        let base = ctx.endpoints.huyaBase.trimmingTrailingSlash
        let url = "\(base)/\(roomId.trimmingCharacters(in: CharacterSet(charactersIn: "/")))"
        let html = try await huyaLsGetText(ctx, url: url, ua: nil)
        guard let n = huyaExtractProfileRoomId(html) else {
            throw LiveKitError.parse("huya: missing profileRoom")
        }
        rid = n
    }

    let mpBase = ctx.endpoints.huyaMpBase.trimmingTrailingSlash
    let mpURL = "\(mpBase)/cache.php?m=Live&do=profileRoom&roomid=\(rid)"
    let uaIPhone = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0 Mobile/15E148 Safari/604.1"
    let json = try await huyaLsGetJSON(ctx, url: mpURL, ua: uaIPhone)
    guard let liveData = json.pointer("/data/liveData") else {
        throw LiveKitError.parse("huya: missing data.liveData")
    }

    let roomName = getStr(liveData, "/roomName") ?? ""
    let intro = getStr(liveData, "/introduction") ?? ""
    let nick = getStr(liveData, "/nick")?.filter { !$0.isWhitespace }
    let avatar = getStr(liveData, "/avatar180")?.filter { !$0.isWhitespace }
    let cover = getStr(liveData, "/screenshot")?.filter { !$0.isWhitespace }
    let title = huyaPickTitle(roomName: roomName, intro: intro, fallback: nick ?? "")
    let liveStatus = getStr(json, "/data/liveStatus") ?? ""
    let isLiving = liveStatus == "ON"

    let canonicalRid = getI64(liveData, "/profileRoom")
        ?? getStr(liveData, "/profileRoom").flatMap { Int64($0) }
        ?? rid

    let info = LiveInfo(title: title, name: nick, avatar: avatar, cover: cover, isLiving: isLiving)

    var variants: [StreamVariant] = []
    if isLiving {
        let bitRateInfoStr = getStr(liveData, "/bitRateInfo") ?? ""
        let brs = huyaParseBitrateInfo(bitRateInfoStr)

        var streams = huyaParseStreamInfos(json)
        // 对齐 Rust：优先非 txdirect 主机。
        streams.sort { !$0.flvUrl.contains("txdirect.flv.huya.com") && $1.flvUrl.contains("txdirect.flv.huya.com") }
        let nowMs = ctx.env.nowMs()

        for (label, bitrate) in brs {
            var urls: [String] = streams.compactMap { st in
                HuyaUrl.format(
                    streamName: st.streamName,
                    flvUrl: st.flvUrl,
                    flvSuffix: st.suffix,
                    flvAntiCode: st.antiCode,
                    presenterUid: st.presenterUid,
                    nowMs: nowMs,
                    ratio: bitrate > 0 ? bitrate : nil
                )
            }
            if urls.isEmpty { continue }
            let url = urls.removeFirst()
            let quality = bitrate == 0 ? 9_999_999 : bitrate
            variants.append(StreamVariant(id: huyaVariantId(bitrate: bitrate, label: label), label: label, quality: quality, rate: nil, url: url, backupUrls: urls))
        }
        variants.sort { $0.quality > $1.quality }
    }

    return LiveManifest(
        site: .huya,
        roomId: String(canonicalRid),
        rawInput: rawInput,
        info: info,
        playback: PlaybackHints(
            referer: "https://www.huya.com/",
            userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3.1 Safari/605.1.15"
        ),
        variants: variants
    )
}

func huyaResolveVariant(ctx: LivestreamContext, roomId: String, variantId: String) async throws -> StreamVariant {
    // Huya 通常一步出 URL：重跑 decode 并返回匹配项。
    let man = try await huyaDecodeManifest(ctx: ctx, roomId: roomId, rawInput: roomId, options: .default)
    let vid = variantId.trimmingCharacters(in: .whitespaces)
    guard let firstColon = vid.firstIndex(of: ":") else {
        throw LiveKitError.invalidInput("invalid variant_id")
    }
    let rest = vid[vid.index(after: firstColon)...]
    guard let secondColon = rest.firstIndex(of: ":") else {
        throw LiveKitError.invalidInput("invalid variant_id")
    }
    let bitrateStr = rest[rest.startIndex..<secondColon]
    guard let bitrate = Int(bitrateStr) else {
        throw LiveKitError.invalidInput("invalid bitrate")
    }
    let prefix = "huya:\(bitrate):"
    guard let variant = man.variants.first(where: { $0.id.hasPrefix(prefix) }) else {
        throw LiveKitError.parse("variant not found")
    }
    return variant
}
