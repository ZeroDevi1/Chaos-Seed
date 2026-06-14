import Foundation

/// 解析用户输入（URL / 平台前缀 / 纯数字房间号）为 `(Site, roomId)`。
///
/// 忠实移植 Rust `chaos_core::danmaku::sites::parse_target_hint`。
public enum InputParser {
    public static func parse(_ input: String) throws -> (Site, String) {
        let raw = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty else {
            throw LiveKitError.invalidInput("empty input")
        }

        // 显式前缀：`bilibili:xxx` / `douyu:xxx` / `huya:xxx`（及简写）。
        if let colon = raw.firstIndex(of: ":") {
            let prefix = String(raw[raw.startIndex..<colon]).lowercased()
            let rest = raw[raw.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            if let site = site(forPrefix: prefix) {
                if rest.isEmpty {
                    throw LiveKitError.invalidInput("empty room id")
                }
                // 允许 `site:https://...` 的便捷写法。
                if rest.lowercased().hasPrefix("http://") || rest.lowercased().hasPrefix("https://") {
                    if let firstSeg = firstSegment(ofURLString: rest), !firstSeg.isEmpty {
                        return (site, firstSeg)
                    }
                }
                return (site, rest)
            }
        }

        // URL 输入。
        let lower = raw.lowercased()
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") {
            guard let comps = URLComponents(string: raw) else {
                throw LiveKitError.invalidInput("invalid url")
            }
            let host = (comps.host ?? "").lowercased()
            let firstSeg = firstPathSegment(comps.path)
            guard !firstSeg.isEmpty else {
                throw LiveKitError.invalidInput("missing room id in url: \(raw)")
            }
            if host.hasSuffix("live.bilibili.com") {
                return (.biliLive, firstSeg)
            }
            if host.hasSuffix("douyu.com") {
                if firstSeg.lowercased() == "topic" {
                    throw LiveKitError.invalidInput("unsupported douyu url path: \(raw)")
                }
                return (.douyu, firstSeg)
            }
            if host.hasSuffix("huya.com") {
                return (.huya, firstSeg)
            }
            throw LiveKitError.unsupportedHost(host)
        }

        // 纯数字房间号：默认走 BiliLive。
        if raw.allSatisfy(\.isNumber) {
            return (.biliLive, raw)
        }

        throw LiveKitError.ambiguousInput(raw)
    }

    /// 前缀到 Site 的映射，对齐 Rust（含 `bili`/`dy`/`hy` 简写）。
    private static func site(forPrefix prefix: String) -> Site? {
        switch prefix {
        case "bilibili", "bili", "bl": return .biliLive
        case "douyu", "dy": return .douyu
        case "huya", "hy": return .huya
        default: return nil
        }
    }

    /// 解析完整 URL 字符串，取路径的第一个非空段（去掉首尾 `/`）。
    /// 用于 `site:https://...` 便捷写法。
    private static func firstSegment(ofURLString urlString: String) -> String? {
        guard let comps = URLComponents(string: urlString) else { return nil }
        return firstPathSegment(comps.path)
    }

    /// 取 URL 路径的第一个非空段（去掉首尾 `/`）。
    private static func firstPathSegment(_ path: String) -> String {
        let trimmed = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let first = trimmed.split(separator: "/").first.map(String.init) ?? ""
        return first.trimmingCharacters(in: .whitespaces)
    }
}
