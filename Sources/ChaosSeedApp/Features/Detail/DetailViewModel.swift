import SwiftUI

enum PlaybackLaunchResult {
    case iina
    case iinaNotFound
    case failure(String)
}

/// 详情页视图模型：解析 manifest，管理清晰度选中态与播放器调度。
@MainActor
public final class DetailViewModel: ObservableObject {
    @Published var variants: [StreamVariant] = []
    @Published var selectedVariantId: String?
    @Published var loading = false
    @Published var logs: String = ""

    /// IINA 已启动后，本应用保留详情页用于播放记录与弹幕显示。
    @Published var externalPlaybackActive = false

    /// 当前直播间的弹幕客户端。
    @Published var danmakuClient: DanmakuClient?
    /// 弹幕显示配置。
    @Published var danmakuConfig: DanmakuConfig {
        didSet { Self.saveDanmakuConfig(danmakuConfig) }
    }
    @Published var danmakuConnectionError: String?

    let room: LiveRoomCard
    private let liveKit: LiveKit
    private var loadTask: Task<Void, Never>?
    private var playTask: Task<Void, Never>?
    private var danmakuTask: Task<Void, Never>?
    private var danmakuRoomId: String?
    private var resolvingDanmakuRoomId: String?
    private var resolvedRoomId: String
    /// 解析得到的播放提示（Referer/UA），传给播放器。
    private var playbackHints: PlaybackHints = .init()
    private static let danmakuConfigKey = "danmakuConfig"

    public init(room: LiveRoomCard, liveKit: LiveKit) {
        self.room = room
        self.liveKit = liveKit
        self.danmakuConfig = Self.loadDanmakuConfig()
        self.resolvedRoomId = room.roomId
    }

    func decode() {
        loadTask?.cancel()
        loading = true
        selectedVariantId = nil
        logs = "正在解析直播间信息…\n输入：\(room.input)"
        loadTask = Task { [weak self] in
            guard let self else { return }
            do {
                let manifest = try await self.liveKit.decodeManifest(input: self.room.input, options: .default)
                guard !Task.isCancelled else { return }
                self.variants = manifest.variants
                self.playbackHints = manifest.playback
                self.resolvedRoomId = manifest.roomId
                let defaultVariant = manifest.variants.first
                self.selectedVariantId = defaultVariant?.id
                // 汇总解析结果到日志框。
                var lines: [String] = []
                lines.append("✅ 解析完成：\(manifest.variants.count) 个清晰度/线路")
                lines.append("房间：\(manifest.site.displayName) #\(manifest.roomId) | \(manifest.info.isLiving ? "直播中" : "未开播")")
                if let name = manifest.info.name { lines.append("主播：\(name)") }
                for v in manifest.variants {
                    let urlTag: String
                    urlTag = v.isResolved ? "IINA 可播" : "需二段解析"
                    let qualityTag = v.biliQualityDiagnosticText ?? "quality=\(v.quality)"
                    lines.append("  · \(v.label) (\(qualityTag)) [\(urlTag)]")
                }
                lines.append("Referer：\(manifest.playback.referer ?? "-")")
                self.logs = lines.joined(separator: "\n")
            } catch is CancellationError {
                // 被新请求取消，静默
            } catch {
                guard !Task.isCancelled else { return }
                Log.parsing.error("decodeManifest 失败 input=\(self.room.input)", error: error)
                self.logs = "❌ 解析失败：\(error.localizedDescription)\n输入：\(self.room.input)"
            }
            self.loading = false
        }
    }

    /// 开始播放：统一解析 variant 后交给 IINA；本应用只保留历史记录与弹幕显示。
    func play(onResult: @escaping (PlaybackLaunchResult) -> Void) {
        guard let id = selectedVariantId,
              let variant = variants.first(where: { $0.id == id }) else { return }

        let site = room.site
        let roomId = resolvedRoomId
        let hints = effectivePlaybackHints()
        let needResolve = !variant.isResolved
        logs = "正在获取「\(variant.label)」播放地址…"
        playTask?.cancel()
        stopPlaybackSession()

        playTask = Task { [weak self] in
            guard let self else { return }
            var current = variant
            if needResolve {
                do {
                    current = try await self.liveKit.resolveVariant(site: site, roomId: roomId, variantId: id)
                    guard !Task.isCancelled, self.selectedVariantId == id else { return }
                } catch is CancellationError {
                    return
                } catch {
                    guard !Task.isCancelled, self.selectedVariantId == id else { return }
                    Log.parsing.error("resolveVariant 失败 variant=\(id)", error: error)
                    await MainActor.run {
                        self.logs = "❌ 解析失败：\(error.localizedDescription)"
                        onResult(.failure("解析失败：\(error.localizedDescription)"))
                    }
                    return
                }
            }
            guard !Task.isCancelled, self.selectedVariantId == id else { return }

            if let index = self.variants.firstIndex(where: { $0.id == current.id }) {
                self.variants[index] = current
            }

            await MainActor.run {
                guard let primaryURL = current.allURLs.first.flatMap(URL.init(string:)) else {
                    self.logs = "未能获取播放地址"
                    onResult(.failure("无播放地址"))
                    return
                }
                let configuredPath = UserDefaults.standard.string(forKey: "iinaPath")
                    ?? IINAPlayer.defaultAppPath
                let launcher = IINALauncher(appPath: configuredPath)
                launcher.play(url: primaryURL, hints: hints)
                let result = launcher.lastResult ?? .failure("unknown")
                switch result {
                case .launched:
                    self.externalPlaybackActive = true
                    let qualityTag = current.biliQualityDiagnosticText ?? "quality=\(current.quality)"
                    self.logs = "✅ IINA 已启动：\(self.room.title) / \(current.label)\n清晰度：\(qualityTag)\nURL：\(primaryURL.absoluteString)"
                    self.connectDanmaku(roomId: roomId)
                    Log.player.debug("IINA 启动成功")
                    onResult(.iina)
                case .iinaNotFound:
                    self.externalPlaybackActive = false
                    self.logs = "❌ 未检测到 IINA，请安装后重试。"
                    Log.player.error("IINA 未安装")
                    onResult(.iinaNotFound)
                case .failure(let m):
                    self.externalPlaybackActive = false
                    self.logs = "❌ 启动失败：\(m)"
                    Log.player.error("IINA 启动失败: \(m)")
                    onResult(.failure(m))
                }
            }
        }
    }

    /// 停止本应用持有的播放状态与弹幕连接；不会关闭外部 IINA 进程。
    func stopPlaybackSession() {
        playTask?.cancel()
        playTask = nil
        danmakuTask?.cancel()
        danmakuTask = nil
        danmakuClient?.disconnect()
        danmakuClient = nil
        danmakuRoomId = nil
        resolvingDanmakuRoomId = nil
        danmakuConnectionError = nil
        externalPlaybackActive = false
    }

    /// 按平台解析弹幕连接信息并建立 WebSocket。
    private func connectDanmaku(roomId: String) {
        let rid = roomId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rid.isEmpty else { return }
        if danmakuRoomId == rid, danmakuClient != nil {
            return
        }
        if resolvingDanmakuRoomId == rid, danmakuTask != nil {
            return
        }
        danmakuTask?.cancel()
        danmakuClient?.disconnect()
        danmakuClient = nil
        danmakuRoomId = nil
        resolvingDanmakuRoomId = rid
        danmakuConnectionError = nil
        danmakuTask = Task { [weak self] in
            guard let self else { return }
            do {
                let connection = try await self.liveKit.resolveDanmakuConnection(
                    site: self.room.site,
                    roomId: rid
                )
                guard !Task.isCancelled else { return }
                let client = DanmakuClient(connection: connection)
                self.danmakuClient = client
                self.danmakuRoomId = rid
                self.resolvingDanmakuRoomId = nil
                client.connect()
                Log.network.debug(
                    "danmaku: 正在连接 \(self.room.site.rawKey) 房间 \(connection.roomId)"
                )
            } catch is CancellationError {
                return
            } catch {
                guard !Task.isCancelled else { return }
                self.resolvingDanmakuRoomId = nil
                self.danmakuConnectionError = error.localizedDescription
                Log.network.error("danmaku: 连接信息解析失败 room=\(rid)", error: error)
            }
        }
    }

    /// 计算传给播放器的最终 hints：保留 manifest 的 Referer，并补一个浏览器 UA
    /// （Bili CDN 会校验 UA；Huya 的 UA 已在 manifest.playback 里设过则保留）。
    private func effectivePlaybackHints() -> PlaybackHints {
        let biliUA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
        let ua = playbackHints.userAgent ?? (room.site == .biliLive ? biliUA : nil)
        return PlaybackHints(referer: playbackHints.referer, userAgent: ua)
    }

    private static func loadDanmakuConfig() -> DanmakuConfig {
        guard let data = UserDefaults.standard.data(forKey: danmakuConfigKey),
              let config = try? JSONDecoder().decode(DanmakuConfig.self, from: data)
        else {
            return .default
        }
        return config
    }

    private static func saveDanmakuConfig(_ config: DanmakuConfig) {
        guard let data = try? JSONEncoder().encode(config) else { return }
        UserDefaults.standard.set(data, forKey: danmakuConfigKey)
    }

    func copySelectedUrl() -> String? {
        guard let id = selectedVariantId,
              let variant = variants.first(where: { $0.id == id }) else { return nil }
        return variant.allURLs.first
    }
}
