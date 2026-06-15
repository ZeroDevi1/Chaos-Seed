import Foundation

/// 真实 LiveKit：整合直播目录 + 直播源解析，对齐 Rust 端 `LiveDirectoryClient` + `LivestreamClient`。
///
/// 直接调用 `LiveDirectoryPlatforms` 与 `LivestreamPlatforms`，复用共享的
/// `HTTPClient` / `LiveEndpoints` / `EnvConfig` / `BiliWbi`。
public final class RealLiveKit: LiveKit, @unchecked Sendable {
    public let endpoints: LiveEndpoints
    public let env: EnvConfig
    public let http: HTTPClient
    public let biliWbi: BiliWbi

    public init(
        endpoints: LiveEndpoints = LiveEndpoints(),
        env: EnvConfig? = nil,
        httpConfig: HTTPClient.Config = HTTPClient.Config()
    ) {
        self.endpoints = endpoints
        self.env = env ?? EnvConfig()
        self.http = HTTPClient(config: httpConfig)
        self.biliWbi = BiliWbi(http: http, apiBase: endpoints.biliApiBase, liveBase: endpoints.biliLiveBase)
    }

    private var dirContext: LiveDirectoryPlatformContext {
        LiveDirectoryPlatformContext(http: http, endpoints: endpoints, env: env, biliWbi: biliWbi)
    }
    private var streamContext: LivestreamContext {
        LivestreamContext(http: http, endpoints: endpoints, env: env)
    }

    // MARK: LiveKit（目录）

    public func getCategories(site: Site) async throws -> [LiveCategory] {
        try await LiveDirectoryPlatforms.getCategories(ctx: dirContext, site: site)
    }

    public func getRecommendRooms(site: Site, page: Int) async throws -> LiveRoomList {
        try await LiveDirectoryPlatforms.getRecommendRooms(ctx: dirContext, site: site, page: page)
    }

    public func getCategoryRooms(site: Site, parentId: String?, categoryId: String, page: Int) async throws -> LiveRoomList {
        try await LiveDirectoryPlatforms.getCategoryRooms(ctx: dirContext, site: site, parentId: parentId, categoryId: categoryId, page: page)
    }

    public func searchRooms(site: Site, keyword: String, page: Int) async throws -> LiveRoomList {
        try await LiveDirectoryPlatforms.searchRooms(ctx: dirContext, site: site, keyword: keyword, page: page)
    }

    // MARK: LiveKit（解析）

    public func decodeManifest(input: String, options: ResolveOptions) async throws -> LiveManifest {
        let (site, roomId) = try InputParser.parse(input)
        return try await LivestreamPlatforms.decodeManifest(ctx: streamContext, site: site, roomId: roomId, rawInput: input, options: options)
    }

    public func resolveVariant(site: Site, roomId: String, variantId: String) async throws -> StreamVariant {
        try await LivestreamPlatforms.resolveVariant(ctx: streamContext, site: site, roomId: roomId, variantId: variantId)
    }

    public func resolveDanmakuConnection(site: Site, roomId: String) async throws -> DanmakuConnectionInfo {
        guard site == .biliLive else {
            throw LiveKitError.unsupportedSite
        }
        return try await BiliDanmakuResolver(http: http, wbi: biliWbi).resolve(roomId: roomId)
    }
}
