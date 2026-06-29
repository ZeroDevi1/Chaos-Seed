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
                if let urlString = normalizedURLString(from: rest) {
                    return try parseURL(urlString, expectedSite: site)
                }
                return (site, rest)
            }
        }

        // URL、无 scheme 地址或分享文本中的 URL。
        if let urlString = normalizedURLString(from: raw) {
            return try parseURL(urlString)
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

    private static func parseURL(
        _ urlString: String,
        expectedSite: Site? = nil
    ) throws -> (Site, String) {
        guard let components = URLComponents(string: urlString),
              let rawHost = components.host?.lowercased(),
              !rawHost.isEmpty else {
            throw LiveKitError.invalidInput("invalid url")
        }

        let site: Site
        if host(rawHost, matches: "live.bilibili.com") {
            site = .biliLive
        } else if host(rawHost, matches: "douyu.com") {
            site = .douyu
        } else if host(rawHost, matches: "huya.com") {
            site = .huya
        } else {
            throw LiveKitError.unsupportedHost(rawHost)
        }
        if let expectedSite, expectedSite != site {
            throw LiveKitError.invalidInput(
                "url host does not match \(expectedSite.displayName): \(rawHost)"
            )
        }

        let pathSegments = components.path
            .split(separator: "/")
            .map(String.init)
            .filter { !$0.isEmpty }
        let roomId: String?
        switch site {
        case .biliLive:
            roomId = roomIdForBili(pathSegments, queryItems: components.queryItems ?? [])
        case .douyu:
            roomId = roomIdForDouyu(pathSegments)
        case .huya:
            roomId = roomIdForHuya(pathSegments)
        }

        guard let roomId, !roomId.isEmpty else {
            throw LiveKitError.invalidInput("missing room id in url: \(urlString)")
        }
        return (site, roomId)
    }

    /// 接受完整 URL、常见无 scheme 地址，以及分享文本中首个 HTTP(S) URL。
    private static func normalizedURLString(from input: String) -> String? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let lower = trimmed.lowercased()
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") {
            return trimURLPunctuation(trimmed)
        }

        if let range = trimmed.range(
            of: #"https?://[^\s]+"#,
            options: [.regularExpression, .caseInsensitive]
        ) {
            return trimURLPunctuation(String(trimmed[range]))
        }

        let knownHosts = [
            "live.bilibili.com/",
            "www.douyu.com/",
            "m.douyu.com/",
            "www.huya.com/",
            "m.huya.com/",
        ]
        if knownHosts.contains(where: { lower.hasPrefix($0) }) {
            return "https://\(trimURLPunctuation(trimmed))"
        }
        return nil
    }

    private static func trimURLPunctuation(_ value: String) -> String {
        value.trimmingCharacters(
            in: CharacterSet(charactersIn: " \t\r\n<>[](){}，。！？、；：\"'")
        )
    }

    private static func host(_ host: String, matches domain: String) -> Bool {
        host == domain || host.hasSuffix(".\(domain)")
    }

    private static func roomIdForBili(
        _ segments: [String],
        queryItems: [URLQueryItem]
    ) -> String? {
        if let first = segments.first, first.lowercased() != "h5" {
            return first
        }
        if segments.first?.lowercased() == "h5", segments.count > 1 {
            return segments[1]
        }
        return queryItems
            .first(where: { ["room_id", "roomid"].contains($0.name.lowercased()) })?
            .value
    }

    private static func roomIdForDouyu(_ segments: [String]) -> String? {
        guard let first = segments.first else { return nil }
        let unsupported = Set(["topic", "directory", "search", "member", "gapi"])
        return unsupported.contains(first.lowercased()) ? nil : first
    }

    private static func roomIdForHuya(_ segments: [String]) -> String? {
        guard let first = segments.first else { return nil }
        let unsupported = Set(["g", "l", "search", "video"])
        return unsupported.contains(first.lowercased()) ? nil : first
    }
}
