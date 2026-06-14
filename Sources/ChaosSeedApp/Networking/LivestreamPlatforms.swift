import Foundation

/// 直播源解析平台实现（decode_manifest / resolve_variant）。
///
/// 忠实移植 Rust `chaos_core::livestream::platforms::{bili_live,douyu,huya}.rs`。
public struct LivestreamContext: Sendable {
    public let http: HTTPClient
    public let endpoints: LiveEndpoints
    public let env: EnvConfig

    public init(http: HTTPClient, endpoints: LiveEndpoints, env: EnvConfig) {
        self.http = http
        self.endpoints = endpoints
        self.env = env
    }
}

public enum LivestreamPlatforms {
    public static func decodeManifest(ctx: LivestreamContext, site: Site, roomId: String, rawInput: String, options: ResolveOptions) async throws -> LiveManifest {
        switch site {
        case .biliLive: return try await biliDecodeManifest(ctx: ctx, roomId: roomId, rawInput: rawInput, options: options)
        case .douyu: return try await douyuDecodeManifest(ctx: ctx, roomId: roomId, rawInput: rawInput, options: options)
        case .huya: return try await huyaDecodeManifest(ctx: ctx, roomId: roomId, rawInput: rawInput, options: options)
        }
    }

    public static func resolveVariant(ctx: LivestreamContext, site: Site, roomId: String, variantId: String) async throws -> StreamVariant {
        switch site {
        case .biliLive: return try await biliResolveVariant(ctx: ctx, roomId: roomId, variantId: variantId)
        case .douyu: return try await douyuResolveVariant(ctx: ctx, roomId: roomId, variantId: variantId)
        case .huya: return try await huyaResolveVariant(ctx: ctx, roomId: roomId, variantId: variantId)
        }
    }
}

// MARK: - JSON 辅助（pointer 取值）

func getI64(_ v: JSONValue, _ ptr: String) -> Int64? { v.pointer(ptr)?.asInt64 }
func getStr(_ v: JSONValue, _ ptr: String) -> String? { v.pointer(ptr)?.asString }
func getBool(_ v: JSONValue, _ ptr: String) -> Bool? { v.pointer(ptr)?.asBool }

func biliMakeVariantId(qn: Int, label: String) -> String { "bili_live:\(qn):\(label)" }
func douyuMakeVariantId(rate: Int, label: String) -> String { "douyu:\(rate):\(label)" }
func huyaVariantId(bitrate: Int, label: String) -> String { "huya:\(bitrate):\(label)" }
