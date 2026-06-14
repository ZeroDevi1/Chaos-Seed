import Foundation

/// MBGA：BiliLive CDN URL 排序与去重。
/// 忠实移植 Rust `chaos_core::livestream::util::mbga`。
public enum Mbga {
    /// Bili CDN 优先级（数字越小越优先）。
    private enum BiliCdn: Int, Comparable {
        case mirror = 0
        case cache = 1
        case mcdn = 2
        case pcdn = 3
        static func < (lhs: BiliCdn, rhs: BiliCdn) -> Bool { lhs.rawValue < rhs.rawValue }
    }

    /// 根据 host 判定 CDN 等级。
    private static func cdnLevel(forHost host: String) -> BiliCdn {
        let h = host.lowercased()
        if h.contains(".mcdn.bilivideo.cn") { return .mcdn }
        if h.contains(".szbdyd.com") { return .pcdn }
        if h.contains("bilivideo.com") && h.hasPrefix("up") { return .mirror }
        return .cache
    }

    /// URL 的 CDN 等级；解析失败按 mcdn（最低）处理。
    public static func cdnLevel(_ urlString: String) -> Int {
        guard let url = URL(string: urlString), let host = url.host else {
            return BiliCdn.mcdn.rawValue
        }
        return cdnLevel(forHost: host).rawValue
    }

    /// 去重后按 CDN 偏好排序。对齐 `sort_urls`。
    public static func sortUrls(_ urls: [String]) -> [String] {
        // BTreeSet 去重保持有序，这里用 Set 去重再按 cdnLevel 排序（稳定）。
        var seen = Set<String>()
        var unique: [String] = []
        for u in urls where !seen.contains(u) {
            seen.insert(u)
            unique.append(u)
        }
        return unique.sorted { cdnLevel($0) < cdnLevel($1) }
    }
}

/// 平衡花括号提取（移植 `brace_extract`）。
/// 用于从 HTML/JS 中提取嵌入的 JSON 对象。
public enum BraceExtract {
    /// 在 `marker` 之后提取第一个平衡的 `{...}` 对象。
    public static func extractBalancedObject(afterMarker marker: String, in input: String) -> String? {
        guard let range = input.range(of: marker) else { return nil }
        let after = input[range.upperBound...]
        return extractBalancedObject(in: String(after))
    }

    /// 从 `input` 中第一个 `{` 起提取平衡对象。
    public static func extractBalancedObject(in input: String) -> String? {
        // 找第一个 '{'
        guard let startIdx = input.firstIndex(of: "{") else { return nil }
        let startOffset = input.distance(from: input.startIndex, to: startIdx)

        var depth = 0
        var inStr = false
        var escape = false
        let chars = Array(input)
        var i = startOffset
        var endOffset: Int? = nil

        while i < chars.count {
            let ch = chars[i]
            if escape { escape = false; i += 1; continue }
            if inStr {
                switch ch {
                case "\\": escape = true
                case "\"": inStr = false
                default: break
                }
                i += 1
                continue
            }
            switch ch {
            case "\"": inStr = true
            case "{": depth += 1
            case "}":
                depth -= 1
                if depth == 0 { endOffset = i; break }
            default: break
            }
            if endOffset != nil { break }
            i += 1
        }
        guard let end = endOffset else { return nil }
        return String(chars[startOffset...end])
    }
}

/// Douyu 加密签名（移植 `douyu_auth::DouyuEncryption::auth`）。
public struct DouyuEncryption: Sendable, Equatable {
    public var key: String
    public var randStr: String
    public var encTime: Int
    public var encData: String
    public var isSpecial: Int

    public init(key: String, randStr: String, encTime: Int, encData: String, isSpecial: Int) {
        self.key = key
        self.randStr = randStr
        self.encTime = encTime
        self.encData = encData
        self.isSpecial = isSpecial
    }

    /// 对齐 `DouyuEncryption::auth`：
    /// 迭代 encTime 次 `u = md5(u + key)`，再 `md5(u + key + rid+ts)`（special 时省略 rid+ts）。
    public func auth(rid: String, ts: Int64) -> String {
        var u = randStr
        for _ in 0..<max(encTime, 0) {
            u = Crypto.md5Hex(u + key)
        }
        let o = isSpecial == 1 ? "" : "\(rid)\(ts)"
        return Crypto.md5Hex(u + key + o)
    }
}

/// Huya 直播流 URL 拼接（移植 `huya_url::format`）。
public enum HuyaUrl {
    /// 32 位无符号左旋 8 位。
    private static func rotl32_8(_ v: UInt32) -> UInt32 { (v << 8) | (v >> 24) }

    /// Best-effort percent-decode（保留 '+'）。
    private static func percentDecode(_ s: String) -> String {
        var out = [UInt8]()
        let bytes = Array(s.utf8)
        var i = 0
        while i < bytes.count {
            if bytes[i] == 0x25 /* % */ && i + 2 < bytes.count {
                if let v1 = Self.hexDigit(bytes[i + 1]),
                   let v2 = Self.hexDigit(bytes[i + 2]) {
                    out.append(UInt8((v1 << 4) + v2))
                    i += 3
                    continue
                }
            }
            out.append(bytes[i])
            i += 1
        }
        return String(bytes: out, encoding: .utf8) ?? s
    }

    private static func hexDigit(_ b: UInt8) -> UInt32? {
        switch b {
        case 0x30...0x39: return UInt32(b - 0x30) // 0-9
        case 0x41...0x46: return UInt32(b - 0x41 + 10) // A-F
        case 0x61...0x66: return UInt32(b - 0x61 + 10) // a-f
        default: return nil
        }
    }

    /// 对齐 Dart 的 component 编码：仅保留 unreserved，其余 `%XX`。
    private static func percentEncodeComponent(_ s: String) -> String {
        var out = ""
        for b in s.utf8 {
            if (0x41...0x5A).contains(b) || (0x61...0x7A).contains(b) || (0x30...0x39).contains(b)
                || b == 0x2D || b == 0x2E || b == 0x5F || b == 0x7E /* - . _ ~ */ {
                out.append(Character(UnicodeScalar(b)))
            } else {
                out.append(String(format: "%%%02X", b))
            }
        }
        return out
    }

    /// 从查询串解析 key/value（BTreeMap：按键排序、去重）。
    private static func parseQueryPairsRaw(_ s: String) -> [String: String] {
        var out: [String: String] = [:]
        for part in s.split(separator: "&") {
            let splits = part.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let k = (splits.first.map(String.init) ?? "").trimmingCharacters(in: .whitespaces)
            let v = (splits.count > 1 ? String(splits[1]) : "").trimmingCharacters(in: .whitespaces)
            if !k.isEmpty { out[k] = v }
        }
        return out
    }

    private static func httpsify(_ url: String) -> String {
        url.replacingOccurrences(of: "http://", with: "https://")
    }

    /// 对齐 `huya_url::format`：组装 Huya FLV URL + anti-code 签名。
    /// 返回 nil 表示缺少必需字段（fm / wsTime）。
    public static func format(
        streamName: String,
        flvUrl: String,
        flvSuffix: String,
        flvAntiCode: String,
        presenterUid: UInt32,
        nowMs: Int64,
        ratio: Int? = nil
    ) -> String? {
        let anti = parseQueryPairsRaw(flvAntiCode)
        guard let fmRaw = anti["fm"], let wsTime = anti["wsTime"] else { return nil }
        let ctype = anti["ctype"] ?? ""
        let fs = anti["fs"] ?? ""
        let platformId = Int(anti["t"] ?? "") ?? 0

        let isWap = platformId == 103
        let seqid = Int64(presenterUid) + nowMs
        let secretHash = Crypto.md5Hex("\(seqid)|\(ctype)|\(platformId)")

        let fmDec = percentDecode(fmRaw)
        guard let fmB64 = Crypto.base64DecodeToString(fmDec) else { return nil }
        let secretPrefix = fmB64.split(separator: "_").first.map(String.init)?.trimmingCharacters(in: .whitespaces) ?? ""
        guard !secretPrefix.isEmpty else { return nil }

        let convertUid = rotl32_8(presenterUid)
        let calcUid: UInt32 = isWap ? presenterUid : convertUid

        let secretStr = "\(secretPrefix)_\(calcUid)_\(streamName)_\(secretHash)_\(wsTime)"
        let wsSecret = Crypto.md5Hex(secretStr)

        // 保持插入顺序（Dart map 有序）。
        var pairs: [(String, String)] = []
        pairs.append(("wsSecret", wsSecret))
        pairs.append(("wsTime", wsTime))
        pairs.append(("seqid", String(seqid)))
        pairs.append(("ctype", ctype))
        pairs.append(("ver", "1"))
        pairs.append(("fs", fs))
        pairs.append(("fm", percentEncodeComponent(fmDec)))
        pairs.append(("t", String(platformId)))
        if isWap {
            // 罕见分支，完整实现。
            let trimmed = wsTime.hasPrefix("0x") || wsTime.hasPrefix("0X")
                ? String(wsTime.dropFirst(2))
                : wsTime
            let wsTimeI = Int64(trimmed, radix: 16) ?? 0
            let jitter = Double.random(in: 0..<1)
            let ct = Int64((Double(wsTimeI) + jitter) * 1000.0)
            let uuid = UInt32(((Double(ct % 10_000_000_000) + Double.random(in: 0..<1)) * 1000.0)
                .truncatingRemainder(dividingBy: Double(UInt32.max)))
            pairs.append(("uid", String(presenterUid)))
            pairs.append(("uuid", String(uuid)))
        } else {
            pairs.append(("u", String(convertUid)))
        }
        if let r = ratio, r > 0 {
            pairs.append(("ratio", String(r)))
        }
        pairs.append(("codec", "264"))

        let qs = pairs.map { "\($0.0)=\($0.1)" }.joined(separator: "&")
        let base = httpsify(flvUrl)
        return "\(base)/\(streamName).\(flvSuffix)?\(qs)"
    }
}
