import Foundation

/// LiveKit / 解析相关错误。
///
/// 合并对齐 Rust `chaos_core::live_directory::client::LiveDirectoryError` 与
/// `chaos_core::livestream::client::LivestreamError` 中 macOS 端会用到的变体。
public enum LiveKitError: LocalizedError, Sendable {
    case invalidInput(String)
    case ambiguousInput(String)
    case unsupportedSite
    case unsupportedHost(String)
    case http(String)
    case parse(String)
    case needPassword
    case needLogin

    public var errorDescription: String? {
        switch self {
        case .invalidInput(let m): return "无效输入：\(m)"
        case .ambiguousInput(let input):
            return """
            无法识别输入：\(input)。请提供完整 URL，或使用平台前缀： \
            `bilibili:` / `douyu:` / `huya:`
            """
        case .unsupportedSite: return "不支持的平台"
        case .unsupportedHost(let host): return "不支持的 URL 域名：\(host)"
        case .http(let m): return "网络错误：\(m)"
        case .parse(let m): return "解析错误：\(m)"
        case .needPassword: return "该直播间需要密码"
        case .needLogin: return "该直播间需要登录"
        }
    }
}
