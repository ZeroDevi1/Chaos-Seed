import Foundation

/// 通用 HTTP 客户端，封装 `URLSession`，提供与 Rust `reqwest::Client` 等价的能力：
/// GET（带 query / headers）、POST（form-urlencoded）、text/JSON 取回。
public actor HTTPClient {
    public struct Config: Sendable {
        public var timeout: TimeInterval
        public var userAgent: String
        public init(timeout: TimeInterval = 10, userAgent: String = "chaos-seed/0.1") {
            self.timeout = timeout
            self.userAgent = userAgent
        }
    }

    private let session: URLSession
    public let config: Config

    public init(config: Config = Config()) {
        self.config = config
        let cfg = URLSessionConfiguration.default
        cfg.timeoutIntervalForRequest = config.timeout
        cfg.timeoutIntervalForResource = config.timeout * 2
        cfg.httpShouldSetCookies = false
        cfg.httpCookieAcceptPolicy = .never
        self.session = URLSession(configuration: cfg)
    }

    // MARK: GET

    /// GET 取回文本。`headers` 为额外 HTTP header。
    public func getText(_ urlString: String, query: [(String, String)] = [], headers: [String: String] = [:]) async throws -> String {
        let (data, _) = try await perform(urlString, method: "GET", query: query, headers: headers, body: nil)
        return String(data: data, encoding: .utf8) ?? ""
    }

    /// GET 取回并解析 JSON。
    public func getJSON(_ urlString: String, query: [(String, String)] = [], headers: [String: String] = [:]) async throws -> JSONValue {
        let (data, response) = try await perform(urlString, method: "GET", query: query, headers: headers, body: nil)
        try Self.ensureSuccess(response, data: data)
        let any = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        return JSONValue.fromAny(any)
    }

    // MARK: POST (form)

    /// POST 表单并解析 JSON。对齐 Rust `http.post(url).form(map)`。
    public func postFormJSON(_ urlString: String, form: [(String, String)], headers: [String: String] = [:]) async throws -> JSONValue {
        let body = form.map { "\(Self.urlEncode($0.0))=\(Self.urlEncode($0.1))" }.joined(separator: "&")
        var allHeaders = headers
        allHeaders["Content-Type"] = "application/x-www-form-urlencoded"
        let (data, response) = try await perform(urlString, method: "POST", query: [], headers: allHeaders, body: Data(body.utf8))
        try Self.ensureSuccess(response, data: data)
        let any = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        return JSONValue.fromAny(any)
    }

    // MARK: 内部

    private func perform(
        _ urlString: String,
        method: String,
        query: [(String, String)],
        headers: [String: String],
        body: Data?
    ) async throws -> (Data, HTTPURLResponse) {
        guard var comps = URLComponents(string: urlString) else {
            throw LiveKitError.invalidInput("invalid url: \(urlString)")
        }
        if !query.isEmpty {
            var items = comps.queryItems ?? []
            items.append(contentsOf: query.map { URLQueryItem(name: $0.0, value: $0.1) })
            comps.queryItems = items
        }
        guard let url = comps.url else {
            throw LiveKitError.invalidInput("invalid url: \(urlString)")
        }
        var req = URLRequest(url: url)
        req.httpMethod = method
        req.setValue(config.userAgent, forHTTPHeaderField: "User-Agent")
        for (k, v) in headers { req.setValue(v, forHTTPHeaderField: k) }
        if let body { req.httpBody = body }

        let started = Date()
        Log.network.debug("\(method) \(url.absoluteString)")
        let (data, response) = try await session.data(for: req)
        guard let http = response as? HTTPURLResponse else {
            throw LiveKitError.http("non-http response")
        }
        let elapsedMs = Int(Date().timeIntervalSince(started) * 1000)
        if (200..<300).contains(http.statusCode) {
            Log.network.debug("\(method) \(http.statusCode) \(elapsedMs)ms \(url.absoluteString)")
        } else {
            // 非 2xx 在 ensureSuccess 抛错前先记录，便于定位（带 body 片段）。
            let snippet = String(data: data.prefix(160), encoding: .utf8) ?? ""
            Log.network.error("\(method) \(url.absoluteString) -> \(http.statusCode) (\(elapsedMs)ms) body=\(snippet)")
        }
        return (data, http)
    }

    private static func ensureSuccess(_ response: HTTPURLResponse, data: Data) throws {
        guard (200..<300).contains(response.statusCode) else {
            let snippet = String(data: data.prefix(200), encoding: .utf8) ?? ""
            throw LiveKitError.http("status \(response.statusCode): \(snippet)")
        }
    }

    /// application/x-www-form-urlencoded 编码。
    static func urlEncode(_ s: String) -> String {
        // 对齐 reqwest 的 form 编码：空格 → +，其余按需。
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-_.~")
        return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? s
    }
}

extension JSONValue {
    /// 从任意 `JSONSerialization` 结果构造（fileprivate 包装，供 HTTPClient 使用）。
    static func fromAny(_ any: Any) -> JSONValue {
        switch any {
        case is NSNull: return .null
        case let n as NSNumber:
            let type = String(cString: n.objCType)
            if type == "c" || type == "B" {
                return .bool(n.boolValue)
            }
            return .number(n.doubleValue)
        case let s as String: return .string(s)
        case let a as [Any]: return .array(a.map(JSONValue.fromAny))
        case let d as [String: Any]: return .object(d.mapValues(JSONValue.fromAny))
        default: return .null
        }
    }
}
