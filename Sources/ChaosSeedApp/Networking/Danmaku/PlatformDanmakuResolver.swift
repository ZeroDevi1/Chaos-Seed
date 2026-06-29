import Foundation

enum PlatformDanmakuResolver {
    private static let huyaUserAgent =
        "Mozilla/5.0 (iPhone; CPU iPhone OS 13_2_3 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148"

    static func resolveDouyu(http: HTTPClient, roomId: String) async throws -> DanmakuConnectionInfo {
        let input = roomId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !input.isEmpty else {
            throw LiveKitError.invalidInput("empty douyu room id")
        }
        let html = try await http.getText(
            "https://www.douyu.com/\(input)",
            headers: [
                "Referer": "https://www.douyu.com/",
                "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Safari/537.36",
            ]
        )
        let canonicalRoomId = try parseDouyuRoomId(html)
        return DanmakuConnectionInfo(
            site: .douyu,
            roomId: canonicalRoomId,
            endpoint: URL(string: "wss://danmuproxy.douyu.com:8506/")!
        )
    }

    static func resolveHuya(http: HTTPClient, roomId: String) async throws -> DanmakuConnectionInfo {
        let input = roomId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !input.isEmpty else {
            throw LiveKitError.invalidInput("empty huya room id")
        }
        let html = try await http.getText(
            "https://m.huya.com/\(input)",
            headers: ["User-Agent": huyaUserAgent]
        )
        let jsonText = try extractHuyaGlobalInit(html)
        let json = try JSONValue(parsing: jsonText)
        guard let yyuid = json.pointer("/roomInfo/tLiveInfo/lYyid")?.asInt64,
              let uid = json.pointer("/roomInfo/tLiveInfo/lUid")?.asInt64 else {
            throw LiveKitError.parse("huya danmaku room metadata missing")
        }
        return DanmakuConnectionInfo(
            site: .huya,
            roomId: input,
            endpoint: URL(string: "wss://cdnws.api.huya.com")!,
            huyaYyuid: yyuid,
            huyaUid: uid
        )
    }

    static func parseDouyuRoomId(_ html: String) throws -> String {
        for marker in ["\\\"roomInfo\\\"", "\"roomInfo\"", "roomInfo"] {
            guard let object = BraceExtract.extractBalancedObject(afterMarker: marker, in: html) else {
                continue
            }
            var unescaped = object
            for _ in 0..<2 {
                unescaped = unescaped.replacingOccurrences(of: "\\\"", with: "\"")
            }
            if let json = try? JSONValue(parsing: unescaped),
               let value = json.pointer("/room/room_id") {
                if let roomId = value.asInt64 {
                    return String(roomId)
                }
                if let roomId = value.asString, !roomId.isEmpty {
                    return roomId
                }
            }
        }
        for pattern in [
            #"window\.room_id\s*=\s*(\d+)"#,
            #"room_id\\?"?\s*[:=]\s*"?(\d+)"?"#,
        ] {
            guard let expression = try? NSRegularExpression(pattern: pattern),
                  let match = expression.firstMatch(
                    in: html,
                    range: NSRange(html.startIndex..., in: html)
                  ),
                  let range = Range(match.range(at: 1), in: html) else {
                continue
            }
            return String(html[range])
        }
        throw LiveKitError.parse("douyu danmaku room id missing")
    }

    static func extractHuyaGlobalInit(_ html: String) throws -> String {
        guard let assignment = html.range(of: "window.HNF_GLOBAL_INIT"),
              let object = BraceExtract.extractBalancedObject(in: String(html[assignment.lowerBound...]))
        else {
            throw LiveKitError.parse("huya HNF_GLOBAL_INIT missing")
        }
        return object
    }
}
