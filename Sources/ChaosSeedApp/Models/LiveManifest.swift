import Foundation

/// 直播间元信息。
///
/// 对齐 Rust `chaos_core::livestream::model::LiveInfo`。
public struct LiveInfo: Codable, Hashable, Sendable {
    public var title: String
    public var name: String?
    public var avatar: String?
    public var cover: String?
    public var isLiving: Bool

    public init(title: String, name: String? = nil, avatar: String? = nil, cover: String? = nil, isLiving: Bool = false) {
        self.title = title
        self.name = name
        self.avatar = avatar
        self.cover = cover
        self.isLiving = isLiving
    }
}

/// 播放所需的 HTTP header 提示（Referer / UA）。
///
/// 对齐 Rust `chaos_core::livestream::model::PlaybackHints`。
/// IINA 启动时会把这些 header 通过 `--mpv-http-header-fields` 传入。
public struct PlaybackHints: Codable, Hashable, Sendable {
    public var referer: String?
    public var userAgent: String?

    public init(referer: String? = nil, userAgent: String? = nil) {
        self.referer = referer
        self.userAgent = userAgent
    }
}

/// 单条清晰度 / 线路。
///
/// 对齐 Rust `chaos_core::livestream::model::StreamVariant`。
/// - `id`：稳定标识，例如 `bili_live:<qn>:<label>`，用于 `resolveVariant`。
/// - `quality`：BiliLive=qn，Huya=bitrate，Douyu=bit。
/// - `url`：可能为空，需要二次调用 `resolveVariant` 获取。
public struct StreamVariant: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var label: String
    public var quality: Int
    public var rate: Int?
    public var url: String?
    public var backupUrls: [String]

    public init(id: String, label: String, quality: Int, rate: Int? = nil, url: String? = nil, backupUrls: [String] = []) {
        self.id = id
        self.label = label
        self.quality = quality
        self.rate = rate
        self.url = url
        self.backupUrls = backupUrls
    }

    /// 是否已带可直接播放的直连 URL（未带则需要二段解析）。
    public var isResolved: Bool {
        url?.trimmingCharacters(in: .whitespaces).isEmpty == false
    }
}

/// 解析一个直播间后的完整清单。
///
/// 对齐 Rust `chaos_core::livestream::model::LiveManifest`。
public struct LiveManifest: Codable, Hashable, Sendable {
    public var site: Site
    public var roomId: String
    public var rawInput: String
    public var info: LiveInfo
    public var playback: PlaybackHints
    public var variants: [StreamVariant]

    public init(
        site: Site,
        roomId: String,
        rawInput: String,
        info: LiveInfo,
        playback: PlaybackHints = PlaybackHints(),
        variants: [StreamVariant] = []
    ) {
        self.site = site
        self.roomId = roomId
        self.rawInput = rawInput
        self.info = info
        self.playback = playback
        self.variants = variants
    }
}

/// 解析选项。
///
/// 对齐 Rust `chaos_core::livestream::model::ResolveOptions`。
/// `dropInaccessibleHighQualities`：若已有可播放清晰度，丢弃更高但拿不到 URL 的档位（对齐 IINA+ 行为）。
public struct ResolveOptions: Codable, Hashable, Sendable {
    public var dropInaccessibleHighQualities: Bool

    public init(dropInaccessibleHighQualities: Bool = true) {
        self.dropInaccessibleHighQualities = dropInaccessibleHighQualities
    }

    public static let `default` = ResolveOptions()
}
