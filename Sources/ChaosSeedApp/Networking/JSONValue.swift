import Foundation

/// 轻量 JSON 值类型，对齐 Rust `serde_json::Value` 的用法（pointer / as_i64 / as_str / as_array）。
///
/// 不使用 `Any` 以保持 `Sendable` 与类型安全；提供与 Rust 一致的指针访问 `/a/b/0` 风格。
public enum JSONValue: Sendable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    /// 从 JSON 字符串解析。
    public init(parsing json: String) throws {
        let data = json.data(using: .utf8) ?? Data()
        let any = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        self = JSONValue.fromAny(any)
    }

    // MARK: 类型断言（对齐 Rust as_i64/as_str/as_array/as_bool）

    public var asInt64: Int64? {
        switch self {
        case .number(let d):
            guard d.isFinite else { return nil }
            return Int64(d)
        case .bool(let b): return b ? 1 : 0
        case .string(let s): return Int64(s.trimmingCharacters(in: .whitespaces))
        default: return nil
        }
    }

    public var asString: String? {
        if case .string(let s) = self { return s }
        return nil
    }

    public var asBool: Bool? {
        if case .bool(let b) = self { return b }
        return nil
    }

    public var asArray: [JSONValue]? {
        if case .array(let a) = self { return a }
        return nil
    }

    public var asObject: [String: JSONValue]? {
        if case .object(let o) = self { return o }
        return nil
    }

    // MARK: 指针访问（对齐 serde_json pointer "/a/b/0"）

    /// 用 `/a/b/0` 风格指针取子值。对齐 Rust `Value::pointer`。
    public func pointer(_ ptr: String) -> JSONValue? {
        guard ptr.hasPrefix("/") else { return ptr.isEmpty ? self : nil }
        let comps = ptr.split(separator: "/").map { String($0) }
        var current: JSONValue? = self
        for raw in comps {
            // serde_json 的 unescape：~1 -> /，~0 -> ~
            let key = raw.replacingOccurrences(of: "~1", with: "/")
                .replacingOccurrences(of: "~0", with: "~")
            guard let cur = current else { return nil }
            switch cur {
            case .object(let o):
                current = o[key]
            case .array(let a):
                guard let idx = Int(key), idx >= 0, idx < a.count else { return nil }
                current = a[idx]
            default:
                return nil
            }
        }
        return current
    }
}
