import Foundation

/// 直播平台枚举。
///
/// 对齐 Rust `chaos_core::danmaku::model::Site`。
/// `as_str` 的返回值（`bili_live` / `douyu` / `huya`）即为高保真原型中使用的平台 ID，
/// 也用于 `input` 字段的平台前缀判断。
public enum Site: String, Codable, CaseIterable, Hashable, Sendable, Identifiable {
    case biliLive
    case douyu
    case huya

    public var id: String { rawKey }

    /// 稳定的字符串标识，与 Rust 端 `Site::as_str()` 完全一致。
    public var rawKey: String {
        switch self {
        case .biliLive: return "bili_live"
        case .douyu: return "douyu"
        case .huya: return "huya"
        }
    }

    /// 用户可见的展示名。
    public var displayName: String {
        switch self {
        case .biliLive: return "BiliLive"
        case .douyu: return "Douyu"
        case .huya: return "Huya"
        }
    }

    /// 构造直播间的 `input` 字段（`bilibili:<rid>` / `douyu:<rid>` / `huya:<rid>`）。
    /// 注意：前缀与 `rawKey` 不同——这里使用人类可读的全名前缀，与 Rust `make_input` 一致。
    public func makeInput(roomId: String) -> String {
        switch self {
        case .biliLive: return "bilibili:\(roomId)"
        case .douyu: return "douyu:\(roomId)"
        case .huya: return "huya:\(roomId)"
        }
    }
}
