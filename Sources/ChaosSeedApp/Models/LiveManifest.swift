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
/// IINA 启动时会把这些 header 通过 mpv 的 referrer / user-agent 选项传入。
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
    /// HTTP-FLV 等由内嵌 libmpv 解封装和渲染。
    case libMPV
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

    /// BiliLive CDN 实际下发的 qn，优先读取主播放 URL 的 `qn`，再退到 `expected_qn`。
    ///
    /// B 站会出现“请求 qn=10000（原画），URL 实际 qn=250（720P）”的回退；
    /// 这里仅做展示诊断，不改变解析器选择 URL 的行为。
    public var biliActualQn: Int? {
        guard id.hasPrefix("bili_live:"),
              let primary = allURLs.first,
              let components = URLComponents(string: primary),
              let items = components.queryItems else {
            return nil
        }
        return Self.queryInt(items, named: "qn")
            ?? Self.queryInt(items, named: "expected_qn")
    }

    public var biliActualQualityText: String? {
        guard let qn = biliActualQn else { return nil }
        return Self.biliQnText(qn)
    }

    public var biliQualityDiagnosticText: String? {
        guard id.hasPrefix("bili_live:") else { return nil }
        if let actual = biliActualQn, actual != quality {
            return "请求 \(Self.biliQnText(quality))，实际 \(Self.biliQnText(actual))"
        }
        return "qn=\(quality)"
    }

    private static func queryInt(_ items: [URLQueryItem], named name: String) -> Int? {
        items.first(where: { $0.name == name })?.value.flatMap(Int.init)
    }

    private static func biliQnText(_ qn: Int) -> String {
        switch qn {
        case 30000: return "杜比 (qn=30000)"
        case 20000: return "4K (qn=20000)"
        case 15000: return "2K (qn=15000)"
        case 10000: return "原画 (qn=10000)"
        case 400: return "1080P 蓝光 (qn=400)"
        case 250: return "720P 超清 (qn=250)"
        case 150: return "480P 高清 (qn=150)"
        case 80: return "流畅 (qn=80)"
        default: return "qn=\(qn)"
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

    /// 当前清晰度下全部可用的内置播放候选。
    ///
    /// 与 WinUI3 播放器一致保留主链和备链；macOS 优先尝试系统原生 HLS，
    /// 失败后继续尝试其它 HLS/CDN，最后回退到 WebKit HTTP-FLV。
    public var builtinPlaybackSources: [BuiltinPlaybackSource] {
        let urls = allURLs.compactMap(URL.init(string:))
        let nativeFormats: Set<String> = ["m3u8", "mp4", "m4v", "mov"]
        let native = urls.compactMap { url -> BuiltinPlaybackSource? in
            guard nativeFormats.contains(url.pathExtension.lowercased()) else { return nil }
            return BuiltinPlaybackSource(url: url, engine: .avFoundation)
        }
        let libMPV = urls.compactMap { url -> BuiltinPlaybackSource? in
            guard ["http", "https"].contains(url.scheme?.lowercased() ?? ""),
                  url.pathExtension.lowercased() == "flv" else {
                return nil
            }
            return BuiltinPlaybackSource(url: url, engine: .libMPV)
        }
        let webFLV = urls.compactMap { url -> BuiltinPlaybackSource? in
            guard ["http", "https"].contains(url.scheme?.lowercased() ?? ""),
                  url.pathExtension.lowercased() == "flv" else {
                return nil
            }
            return BuiltinPlaybackSource(url: url, engine: .webFLV)
        }
        return native + libMPV + webFLV
    }

    /// 当前线路首选的内置播放源。
    public var builtinPlaybackSource: BuiltinPlaybackSource? {
        builtinPlaybackSources.first
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
