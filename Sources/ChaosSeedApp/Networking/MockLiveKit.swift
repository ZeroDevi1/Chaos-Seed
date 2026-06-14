import Foundation

/// Mock 实现的 LiveKit。
///
/// 数据与过滤逻辑移植自高保真原型 `designs/ChaosSeed-macOS/data.jsx`，
/// 但采用对齐 Rust chaos-core 的模型字段（`roomId`/`input`/`userName`/`online`），
/// 以便后续真实网络层直接替换。所有方法用 `Task.sleep` 模拟网络延迟。
public final class MockLiveKit: LiveKit, @unchecked Sendable {
    /// 是否在每次调用时模拟网络延迟。
    public var simulateLatency: Bool = true

    public init(simulateLatency: Bool = true) {
        self.simulateLatency = simulateLatency
    }

    // MARK: LiveKit

    public func getCategories(site: Site) async throws -> [LiveCategory] {
        await delay()
        return Self.categories(for: site)
    }

    public func getRecommendRooms(site: Site, page: Int) async throws -> LiveRoomList {
        try await rooms(site: site, page: page, keyword: nil, categoryId: "recommend", subCategoryId: nil)
    }

    public func getCategoryRooms(
        site: Site,
        parentId: String?,
        categoryId: String,
        page: Int
    ) async throws -> LiveRoomList {
        try await rooms(site: site, page: page, keyword: nil, categoryId: categoryId, subCategoryId: nil)
    }

    public func searchRooms(site: Site, keyword: String, page: Int) async throws -> LiveRoomList {
        try await rooms(site: site, page: page, keyword: keyword, categoryId: nil, subCategoryId: nil)
    }

    public func decodeManifest(input: String, options: ResolveOptions) async throws -> LiveManifest {
        await delay(millis: 600)
        let (site, roomId) = try InputParser.parse(input)
        // 模拟：用样本数据生成一个 manifest（不查真实房间）。
        let variants = Self.sampleVariants(site: site)
        return LiveManifest(
            site: site,
            roomId: roomId,
            rawInput: input,
            info: LiveInfo(
                title: "\(site.displayName) 房间 \(roomId)",
                name: "示例主播",
                avatar: nil,
                cover: nil,
                isLiving: true
            ),
            playback: PlaybackHints(
                referer: site == .biliLive ? "https://live.bilibili.com/" : nil,
                userAgent: nil
            ),
            variants: variants
        )
    }

    public func resolveVariant(site: Site, roomId: String, variantId: String) async throws -> StreamVariant {
        await delay(millis: 500)
        guard var v = Self.sampleVariants(site: site).first(where: { $0.id == variantId }) else {
            throw LiveKitError.parse("requested quality not accessible: \(variantId)")
        }
        if v.url == nil {
            // 二段解析后补一个示例直连 URL。
            v.url = "https://cdn.example.\(site.rawKey)/\(roomId)/\(variantId).flv"
        }
        return v
    }

    // MARK: 内部

    private func delay(millis: UInt64 = 350) async {
        guard simulateLatency else { return }
        try? await Task.sleep(nanoseconds: millis * 1_000_000)
    }

    /// 复刻原型 `makeRooms` 的过滤 + 分页逻辑，但产出 Rust 风格的 `LiveRoomCard`。
    private func rooms(
        site: Site,
        page: Int,
        keyword: String?,
        categoryId: String?,
        subCategoryId: String?
    ) async throws -> LiveRoomList {
        await delay()
        var pool = Self.sampleRooms(for: site) // 携带内部 tags 用于过滤

        let kw = keyword?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
        if !kw.isEmpty {
            pool = pool.filter { room in
                room.title.lowercased().contains(kw) || room.userName.lowercased().contains(kw)
            }
        } else if let categoryId, categoryId != "recommend", !categoryId.isEmpty {
            pool = pool.filter { room in
                let tags = room.tags
                if let sub = subCategoryId, !sub.isEmpty {
                    return tags.contains(categoryId) || tags.contains(sub)
                }
                return tags.contains(categoryId)
            }
        }

        let total = pool.count
        let pageSize = 6
        let page = max(1, page)
        let start = (page - 1) * pageSize
        let end = min(start + pageSize, total)
        let paged = (start < end) ? Array(pool[start..<end]) : []

        let cards = paged.map { MockRoom.room($0) }
        let hasMore = start + pageSize < total
        return LiveRoomList(hasMore: hasMore, items: cards)
    }
}

// MARK: - Mock 数据（移植自 data.jsx）

/// 带内部 `tags` 的房间（仅 Mock 用，不暴露到公开模型）。
struct MockRoom {
    let id: String
    let title: String
    let userName: String
    let online: Int
    let tags: [String]
    let site: Site

    /// 转成公开模型 `LiveRoomCard`。
    static func room(_ r: MockRoom) -> LiveRoomCard {
        LiveRoomCard(
            site: r.site,
            roomId: r.id,
            input: r.site.makeInput(roomId: r.id),
            title: r.title,
            cover: nil,
            userName: r.userName,
            online: r.online
        )
    }
}

extension MockLiveKit {
    static func categories(for site: Site) -> [LiveCategory] {
        switch site {
        case .biliLive:
            return [
                LiveCategory(id: "recommend", name: "推荐", children: []),
                LiveCategory(id: "game", name: "网游", children: [
                    .init(id: "lol", parentId: "game", name: "英雄联盟"),
                    .init(id: "dota2", parentId: "game", name: "DOTA2"),
                    .init(id: "cs2", parentId: "game", name: "CS2"),
                ]),
                LiveCategory(id: "mobile", name: "手游", children: [
                    .init(id: "honkai", parentId: "mobile", name: "星穹铁道"),
                    .init(id: "genshin", parentId: "mobile", name: "原神"),
                    .init(id: "mahjong", parentId: "mobile", name: "雀魂"),
                ]),
                LiveCategory(id: "single", name: "单机", children: [
                    .init(id: "host", parentId: "single", name: "主机游戏"),
                    .init(id: "indie", parentId: "single", name: "独立游戏"),
                ]),
                LiveCategory(id: "ent", name: "娱乐", children: [
                    .init(id: "sing", parentId: "ent", name: "唱见"),
                    .init(id: "radio", parentId: "ent", name: "电台"),
                ]),
            ]
        case .douyu:
            return [
                LiveCategory(id: "recommend", name: "推荐", children: []),
                LiveCategory(id: "lol", name: "英雄联盟", children: []),
                LiveCategory(id: "dota2", name: "DOTA2", children: []),
                LiveCategory(id: "cs2", name: "CS2", children: []),
                LiveCategory(id: "mobile", name: "手游", children: [
                    .init(id: "king", parentId: "mobile", name: "王者荣耀"),
                    .init(id: "pubgm", parentId: "mobile", name: "和平精英"),
                ]),
                LiveCategory(id: "ent", name: "娱乐", children: [
                    .init(id: "outdoor", parentId: "ent", name: "户外"),
                    .init(id: "music", parentId: "ent", name: "音乐"),
                ]),
            ]
        case .huya:
            return [
                LiveCategory(id: "recommend", name: "推荐", children: []),
                LiveCategory(id: "1", name: "网游", children: [
                    .init(id: "lol", parentId: "1", name: "英雄联盟"),
                    .init(id: "dnf", parentId: "1", name: "DNF"),
                ]),
                LiveCategory(id: "2", name: "单机", children: [
                    .init(id: "mc", parentId: "2", name: "我的世界"),
                    .init(id: "pubg", parentId: "2", name: "绝地求生"),
                ]),
                LiveCategory(id: "3", name: "手游", children: [
                    .init(id: "king", parentId: "3", name: "王者荣耀"),
                    .init(id: "codm", parentId: "3", name: "使命召唤手游"),
                ]),
                LiveCategory(id: "8", name: "娱乐", children: [
                    .init(id: "sing", parentId: "8", name: "星秀"),
                    .init(id: "talk", parentId: "8", name: "脱口秀"),
                ]),
            ]
        }
    }

    static func sampleRooms(for site: Site) -> [MockRoom] {
        switch site {
        case .biliLive:
            return [
                .init(id: "b1", title: "【星穹铁道】新版本主线开荒", userName: "老沐", online: 124_300, tags: ["mobile", "honkai"], site: .biliLive),
                .init(id: "b2", title: "雀魂麻将 段位场打工", userName: "杆菌无敌", online: 8_720, tags: ["mobile", "mahjong"], site: .biliLive),
                .init(id: "b3", title: "炉石传说 竞技场12胜", userName: "异灵术老师", online: 156_000, tags: ["game"], site: .biliLive),
                .init(id: "b4", title: "深夜歌回 / 点歌台", userName: "泠鸢yousa", online: 42_100, tags: ["ent", "sing"], site: .biliLive),
                .init(id: "b5", title: "CS2 完美S局", userName: "玩机器Machine", online: 38_900, tags: ["game", "cs2"], site: .biliLive),
                .init(id: "b6", title: "原神 每日+深渊", userName: "棉花大哥哥", online: 21_500, tags: ["mobile", "genshin"], site: .biliLive),
                .init(id: "b7", title: "英雄联盟 韩服王者局", userName: "Uzi", online: 456_000, tags: ["game", "lol"], site: .biliLive),
                .init(id: "b8", title: "DOTA2 路人单排", userName: "Sccc", online: 32_100, tags: ["game", "dota2"], site: .biliLive),
            ]
        case .douyu:
            return [
                .init(id: "d1", title: "LPL 夏季赛 官方直播间", userName: "英雄联盟赛事", online: 2_100_000, tags: ["lol"], site: .douyu),
                .init(id: "d2", title: "DOTA2 主播带你看比赛", userName: "Zard", online: 56_200, tags: ["dota2"], site: .douyu),
                .init(id: "d3", title: "街头霸王6 练习房", userName: "小孩曾卓君", online: 28_900, tags: [], site: .douyu),
                .init(id: "d4", title: "主机游戏 新作试玩", userName: "女流66", online: 134_000, tags: [], site: .douyu),
                .init(id: "d5", title: "户外直播 城市探索", userName: "彡彡九户外", online: 7_600, tags: ["ent", "outdoor"], site: .douyu),
                .init(id: "d6", title: "王者荣耀 巅峰赛", userName: "张大仙", online: 980_000, tags: ["mobile", "king"], site: .douyu),
            ]
        case .huya:
            return [
                .init(id: "h1", title: "穿越火线 CFPL 职业联赛", userName: "CFPL", online: 430_000, tags: ["1"], site: .huya),
                .init(id: "h2", title: "绝地求生 PCL 训练赛", userName: "4AM战队", online: 125_000, tags: ["2", "pubg"], site: .huya),
                .init(id: "h3", title: "云顶之弈 新版本上分", userName: "神超", online: 67_000, tags: ["1", "lol"], site: .huya),
                .init(id: "h4", title: "永劫无间 三排冲榜", userName: "法神", online: 43_200, tags: ["1"], site: .huya),
                .init(id: "h5", title: "FIFA Online 4 排位", userName: "A胖", online: 18_900, tags: ["1"], site: .huya),
                .init(id: "h6", title: "魔兽争霸3 怀旧对战", userName: "infi", online: 54_300, tags: ["2", "mc"], site: .huya),
            ]
        }
    }

    /// 移植原型 `makeQualities`，但用 Rust 风格的 `StreamVariant` 表达。
    static func sampleVariants(site: Site) -> [StreamVariant] {
        // (label, quality, resolved)
        let defs: [(String, Int, Bool)] = [
            ("原画", 2000, true),
            ("蓝光", 1000, true),
            ("超清", 400, true),
            ("高清", 250, false),
            ("流畅", 80, true),
        ]
        return defs.enumerated().map { idx, d in
            let id = "\(site.rawKey):\(d.1):\(d.0)"
            return StreamVariant(
                id: id,
                label: d.0,
                quality: d.1,
                url: d.2 ? "https://cdn.example.\(site.rawKey)/line\(idx % 3 + 1).flv" : nil,
                backupUrls: []
            )
        }
    }
}
