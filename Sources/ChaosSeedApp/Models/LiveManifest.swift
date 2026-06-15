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

/// 内置播放器实际采用的媒体后端。
public enum BuiltinPlaybackEngine: String, Codable, Hashable, Sendable {
    /// HLS / MP4 等由 AVFoundation 原生解封装。
    case avFoundation
    /// HTTP-FLV 由应用内 WebKit + Media Source Extensions 解封装。
    case webFLV
}

/// 已选定的内置播放源，避免把 HTTP 可访问误判为 AVFoundation 可播放。
public struct BuiltinPlaybackSource: Hashable, Sendable {
    public var url: URL
    public var engine: BuiltinPlaybackEngine

    public init(url: URL, engine: BuiltinPlaybackEngine) {
        self.url = url
        self.engine = engine
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

    /// 主线路与备用线路的去重列表，保持解析器给出的优先级。
    public var allURLs: [String] {
        var seen = Set<String>()
        return ([url].compactMap { $0 } + backupUrls).filter { value in
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            return !trimmed.isEmpty && seen.insert(trimmed).inserted
        }
    }

    /// AVPlayer 可直接尝试的 URL。
    ///
    /// 三个平台的传统 HTTP-FLV 与 P2P `.xs` 线路不能由 AVPlayer 解封装；
    /// 解析器会把 HLS 放入备用地址，点播文件仅接受系统明确支持的扩展名。
    public var builtinPlaybackURL: URL? {
        let urls = allURLs.compactMap(URL.init(string:))
        if let hls = urls.first(where: { $0.pathExtension.lowercased() == "m3u8" }) {
            return hls
        }
        let fileFormats: Set<String> = ["mp4", "m4v", "mov"]
        return urls.first(where: { fileFormats.contains($0.pathExtension.lowercased()) })
    }

    /// 应用内 WebKit 后端可播放的 HTTP-FLV；P2P `.xs` 不属于 HTTP-FLV。
    public var webFLVPlaybackURL: URL? {
        allURLs
            .compactMap(URL.init(string:))
            .first {
                ["http", "https"].contains($0.scheme?.lowercased() ?? "")
                    && $0.pathExtension.lowercased() == "flv"
            }
    }

    /// 当前线路可用的内置播放源，优先选择系统原生 AVFoundation。
    public var builtinPlaybackSource: BuiltinPlaybackSource? {
        if let url = builtinPlaybackURL {
            return BuiltinPlaybackSource(url: url, engine: .avFoundation)
        }
        if let url = webFLVPlaybackURL {
            return BuiltinPlaybackSource(url: url, engine: .webFLV)
        }
        return nil
    }

    /// BiliLive / Huya 优先二次解析出 HLS；Douyu 的官方网页接口直接返回 HTTP-FLV。
    public func needsBuiltinResolution(for site: Site) -> Bool {
        switch site {
        case .biliLive, .huya:
            return builtinPlaybackURL == nil
        case .douyu:
            return builtinPlaybackSource == nil
        }
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
