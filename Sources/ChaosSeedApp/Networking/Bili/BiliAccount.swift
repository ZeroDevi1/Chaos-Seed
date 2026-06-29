import Foundation
import Security

public struct BiliAccount: Codable, Equatable, Sendable {
    public var uid: Int64
    public var name: String

    public init(uid: Int64, name: String) {
        self.uid = uid
        self.name = name
    }
}

public struct BiliQRCodeChallenge: Equatable, Sendable {
    public var url: String
    public var key: String

    public init(url: String, key: String) {
        self.url = url
        self.key = key
    }
}

public enum BiliQRCodePollState: Equatable, Sendable {
    case waiting
    case scanned
    case expired
    case success(cookie: String)
}

public enum BiliAccountStore {
    private static let service = "com.zerodevi1.chaosseed.bilibili"
    private static let account = "cookie"
    private static let accountInfoKey = "biliAccountInfo"

    public static func loadCookie() -> String? {
        var query = baseQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess,
              let data = item as? Data,
              let cookie = String(data: data, encoding: .utf8),
              !cookie.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        return cookie
    }

    public static func saveCookie(_ cookie: String) throws {
        let trimmed = cookie.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let data = Data(trimmed.utf8)
        var query = baseQuery()
        let update: [String: Any] = [
            kSecValueData as String: data,
        ]
        let status = SecItemUpdate(query as CFDictionary, update as CFDictionary)
        if status == errSecSuccess { return }
        guard status == errSecItemNotFound else { throw keychainError(status) }
        query[kSecValueData as String] = data
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addStatus = SecItemAdd(query as CFDictionary, nil)
        guard addStatus == errSecSuccess else { throw keychainError(addStatus) }
    }

    public static func deleteCookie() throws {
        let status = SecItemDelete(baseQuery() as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw keychainError(status)
        }
        UserDefaults.standard.removeObject(forKey: accountInfoKey)
    }

    public static func loadAccount() -> BiliAccount? {
        guard let data = UserDefaults.standard.data(forKey: accountInfoKey) else { return nil }
        return try? JSONDecoder().decode(BiliAccount.self, from: data)
    }

    public static func saveAccount(_ account: BiliAccount) {
        guard let data = try? JSONEncoder().encode(account) else { return }
        UserDefaults.standard.set(data, forKey: accountInfoKey)
    }

    public static func combinedCookie(buvidCookie: String?) -> String? {
        combinedCookie(loginCookie: loadCookie(), buvidCookie: buvidCookie)
    }

    static func combinedCookie(loginCookie: String?, buvidCookie: String?) -> String? {
        let login = normalizedCookie(loginCookie)
        let buvid = normalizedCookie(buvidCookie)
        guard let login else { return buvid }
        guard let buvid else { return login }
        let loginNames = cookieNames(login)
        let extra = buvid
            .split(separator: ";")
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { part in
                guard let name = part.split(separator: "=", maxSplits: 1).first else { return false }
                return !loginNames.contains(String(name))
            }
        guard !extra.isEmpty else { return login }
        return ([login] + extra).joined(separator: ";")
    }

    static func cookieString(from headerFields: [String: String]) -> String {
        let cookieURL = URL(string: "https://bilibili.com")!
        let cookies = HTTPCookie.cookies(withResponseHeaderFields: headerFields, for: cookieURL)
            .map { "\($0.name)=\($0.value)" }
        if !cookies.isEmpty {
            return cookies.joined(separator: ";")
        }
        guard let raw = headerFields.first(where: { $0.key.lowercased() == "set-cookie" })?.value else {
            return ""
        }
        return raw
            .components(separatedBy: ", ")
            .compactMap { segment -> String? in
                let pair = segment.split(separator: ";", maxSplits: 1).first.map(String.init) ?? ""
                return pair.contains("=") ? pair : nil
            }
            .joined(separator: ";")
    }

    private static func baseQuery() -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    private static func normalizedCookie(_ cookie: String?) -> String? {
        let value = cookie?
            .split(separator: ";")
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: ";")
        return value?.isEmpty == false ? value : nil
    }

    private static func cookieNames(_ cookie: String) -> Set<String> {
        Set(cookie.split(separator: ";").compactMap { part in
            part.split(separator: "=", maxSplits: 1).first.map { String($0).trimmingCharacters(in: .whitespaces) }
        })
    }

    private static func keychainError(_ status: OSStatus) -> Error {
        LiveKitError.http("keychain error: \(status)")
    }
}

public final class BiliAccountClient: Sendable {
    private let http: HTTPClient
    private let passportBase: String
    private let apiBase: String

    public init(
        http: HTTPClient = HTTPClient(config: .init(timeout: 15, userAgent: "chaos-seed/0.1")),
        passportBase: String = "https://passport.bilibili.com",
        apiBase: String = "https://api.bilibili.com"
    ) {
        self.http = http
        self.passportBase = passportBase
        self.apiBase = apiBase
    }

    public func generateQRCode() async throws -> BiliQRCodeChallenge {
        let json = try await http.getJSON("\(passportBase)/x/passport-login/web/qrcode/generate")
        guard (json.pointer("/code")?.asInt64 ?? -1) == 0 else {
            throw LiveKitError.http(json.pointer("/message")?.asString ?? "bilibili qrcode generate failed")
        }
        let url = json.pointer("/data/url")?.asString ?? ""
        let key = json.pointer("/data/qrcode_key")?.asString ?? ""
        guard !url.isEmpty, !key.isEmpty else {
            throw LiveKitError.parse("bilibili qrcode missing")
        }
        return BiliQRCodeChallenge(url: url, key: key)
    }

    public func pollQRCode(key: String) async throws -> BiliQRCodePollState {
        let response = try await http.getJSONWithHeaders(
            "\(passportBase)/x/passport-login/web/qrcode/poll",
            query: [("qrcode_key", key)]
        )
        guard (response.json.pointer("/code")?.asInt64 ?? -1) == 0 else {
            throw LiveKitError.http(response.json.pointer("/message")?.asString ?? "bilibili qrcode poll failed")
        }
        let code = response.json.pointer("/data/code")?.asInt64 ?? -1
        switch code {
        case 0:
            let cookie = BiliAccountStore.cookieString(from: response.headers)
            guard !cookie.isEmpty else { throw LiveKitError.parse("bilibili login cookie missing") }
            return .success(cookie: cookie)
        case 86038:
            return .expired
        case 86090:
            return .scanned
        case 86101:
            return .waiting
        default:
            return .waiting
        }
    }

    public func fetchAccount(cookie: String) async throws -> BiliAccount {
        let json = try await http.getJSON(
            "\(apiBase)/x/member/web/account",
            headers: [
                "Cookie": cookie,
                "Referer": "https://www.bilibili.com/",
            ]
        )
        guard (json.pointer("/code")?.asInt64 ?? -1) == 0 else {
            throw LiveKitError.http(json.pointer("/message")?.asString ?? "bilibili account unavailable")
        }
        let uid = json.pointer("/data/mid")?.asInt64 ?? 0
        let name = json.pointer("/data/uname")?.asString ?? ""
        guard uid > 0, !name.isEmpty else {
            throw LiveKitError.parse("bilibili account info missing")
        }
        return BiliAccount(uid: uid, name: name)
    }
}
