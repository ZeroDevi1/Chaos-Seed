import Foundation

/// 统一的直播目录 + 直播源解析门面。
///
/// 合并 Rust 端两个 client 的能力：
/// - `chaos_core::live_directory::client::LiveDirectoryClient`
/// - `chaos_core::livestream::client::LivestreamClient`
///
/// 设计为协议，便于 `MockLiveKit`（v1）与未来的真实实现（BiliLive 优先）零成本替换。
public protocol LiveKit: Sendable {
    // MARK: 目录浏览（对齐 LiveDirectoryClient）

    func getCategories(site: Site) async throws -> [LiveCategory]
    func getRecommendRooms(site: Site, page: Int) async throws -> LiveRoomList
    func getCategoryRooms(
        site: Site,
        parentId: String?,
        categoryId: String,
        page: Int
    ) async throws -> LiveRoomList
    func searchRooms(site: Site, keyword: String, page: Int) async throws -> LiveRoomList

    // MARK: 直播源解析（对齐 LivestreamClient）

    func decodeManifest(input: String, options: ResolveOptions) async throws -> LiveManifest
    func resolveVariant(site: Site, roomId: String, variantId: String) async throws -> StreamVariant
    func resolveDanmakuConnection(site: Site, roomId: String) async throws -> DanmakuConnectionInfo
}

/// 便捷方法：根据是否传 `categoryId` 决定走推荐还是分类。
public extension LiveKit {
    func resolveDanmakuConnection(site: Site, roomId: String) async throws -> DanmakuConnectionInfo {
        throw LiveKitError.unsupportedSite
    }

    func getRooms(
        site: Site,
        categoryId: String?,
        parentId: String?,
        page: Int
    ) async throws -> LiveRoomList {
        guard let categoryId, categoryId != "recommend", !categoryId.isEmpty else {
            return try await getRecommendRooms(site: site, page: page)
        }
        return try await getCategoryRooms(site: site, parentId: parentId, categoryId: categoryId, page: page)
    }
}
