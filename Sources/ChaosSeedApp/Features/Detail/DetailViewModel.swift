import SwiftUI

enum PlaybackLaunchResult {
    case builtin
    case iina
    case iinaNotFound
    case unsupportedBuiltin
    case failure(String)
}

/// 详情页视图模型：解析 manifest，管理清晰度选中态与播放器调度。
@MainActor
public final class DetailViewModel: ObservableObject {
    @Published var variants: [StreamVariant] = []
    @Published var selectedVariantId: String?
    @Published var loading = false
    @Published var logs: String = ""

    /// 内置播放器实例（仅当用户选择内置播放器时创建并持有）。
    @Published var builtinPlayer: BuiltinPlayer?

    /// 弹幕客户端（仅 BiliLive 房间支持）。
    @Published var danmakuClient: DanmakuClient?
    /// 弹幕显示配置。
    @Published var danmakuConfig: DanmakuConfig {
        didSet { Self.saveDanmakuConfig(danmakuConfig) }
    }

    /// 播放器偏好：`builtin` 使用内置 AVPlayer，`iina` 唤起外部 IINA。
    /// 持久化在 UserDefaults 中（`@AppStorage`）。
    @AppStorage("playerPreference") var playerPreferenceRaw = PlayerPreference.builtin.rawValue

    var playerPreference: PlayerPreference {
        get { PlayerPreference(rawValue: playerPreferenceRaw) ?? .builtin }
        set { playerPreferenceRaw = newValue.rawValue }
    }

    let room: LiveRoomCard
    private let liveKit: LiveKit
    private var loadTask: Task<Void, Never>?
    private var playTask: Task<Void, Never>?
    private var danmakuTask: Task<Void, Never>?
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
                let defaultVariant: StreamVariant?
                if self.playerPreference == .builtin {
                    defaultVariant = manifest.variants.first(where: { $0.builtinPlaybackSource != nil })
                        ?? manifest.variants.first
                } else {
                    defaultVariant = manifest.variants.first
                }
                self.selectedVariantId = defaultVariant?.id
                // 汇总解析结果到日志框。
                var lines: [String] = []
                lines.append("✅ 解析完成：\(manifest.variants.count) 个清晰度/线路")
                lines.append("房间：\(manifest.site.displayName) #\(manifest.roomId) | \(manifest.info.isLiving ? "直播中" : "未开播")")
                if let name = manifest.info.name { lines.append("主播：\(name)") }
                for v in manifest.variants {
                    let urlTag: String
                    if let source = v.builtinPlaybackSource {
                        let backend = source.engine == .avFoundation ? "AVFoundation" : "HTTP-FLV"
                        urlTag = "✓内置可播/\(backend)"
                    } else if v.isResolved {
                        urlTag = "FLV / IINA"
                    } else {
                        urlTag = "需二段解析"
                    }
                    lines.append("  · \(v.label) (qn=\(v.quality)) [\(urlTag)]")
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

    /// 开始播放：根据 `playerPreference` 选择内置播放器或 IINA。
    ///
    /// - 内置播放器：创建 `BuiltinPlayer`，解析 variant → 设置 `builtinPlayer` 并发布 `isPlaying` 变化。
    /// - IINA：解析 variant → 调用 `IINALauncher.play()` → 通过回调回报结果。
    func play(
        using overridePreference: PlayerPreference? = nil,
        onResult: @escaping (PlaybackLaunchResult) -> Void
    ) {
        guard let id = selectedVariantId,
              let variant = variants.first(where: { $0.id == id }) else { return }

        let site = room.site
        let roomId = resolvedRoomId
        let hints = effectivePlaybackHints()
        let preference = overridePreference ?? playerPreference
        let needResolve = preference == .builtin
            ? variant.needsBuiltinResolution(for: site)
            : !variant.isResolved
        logs = "正在获取「\(variant.label)」播放地址…"
        playTask?.cancel()
        if preference == .iina {
            stopBuiltinPlayer()
        }

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
                    if preference == .builtin, variant.builtinPlaybackSource != nil {
                        Log.parsing.error("resolveVariant 失败，回退已有内置播放源 variant=\(id)", error: error)
                        current = variant
                    } else {
                        Log.parsing.error("resolveVariant 失败 variant=\(id)", error: error)
                        await MainActor.run {
                            self.logs = "❌ 解析失败：\(error.localizedDescription)"
                            onResult(.failure("解析失败：\(error.localizedDescription)"))
                        }
                        return
                    }
                }
            }
            guard !Task.isCancelled, self.selectedVariantId == id else { return }

            if let index = self.variants.firstIndex(where: { $0.id == current.id }) {
                self.variants[index] = current
            }

            guard let primaryURL = current.allURLs.first.flatMap(URL.init(string:)) else {
                await MainActor.run {
                    self.logs = "未能获取播放地址"
                    onResult(.failure("无播放地址"))
                }
                return
            }

            await MainActor.run {
                switch preference {
                case .builtin:
                    guard let source = current.builtinPlaybackSource else {
                        self.logs = """
                        ❌ 当前清晰度没有可用的 HLS、MP4 或 HTTP-FLV 地址。
                        P2P `.xs` 线路暂不受内置播放器支持，请改用 IINA。
                        """
                        onResult(.unsupportedBuiltin)
                        return
                    }
                    let player: BuiltinPlayer
                    if let currentPlayer = self.builtinPlayer {
                        player = currentPlayer
                    } else {
                        player = BuiltinPlayer()
                        self.builtinPlayer = player
                    }
                    let backend = source.engine == .avFoundation ? "AVFoundation" : "WebKit HTTP-FLV"
                    self.logs = "✅ 内置播放器（\(backend)）：\(self.room.title) / \(current.label)\nURL：\(source.url.absoluteString)"
                    player.play(source: source, hints: hints)
                    self.connectDanmaku(roomId: roomId)
                    onResult(.builtin)
                case .iina:
                    let configuredPath = UserDefaults.standard.string(forKey: "iinaPath")
                        ?? IINAPlayer.defaultAppPath
                    let launcher = IINALauncher(appPath: configuredPath)
                    launcher.play(url: primaryURL, hints: hints)
                    let result = launcher.lastResult ?? .failure("unknown")
                    switch result {
                    case .launched:
                        self.logs = "✅ IINA 已启动：\(self.room.title) / \(current.label)\nURL：\(primaryURL.absoluteString)"
                        Log.player.debug("IINA 启动成功")
                        onResult(.iina)
                    case .iinaNotFound:
                        self.logs = "❌ 未检测到 IINA，请安装后重试。"
                        Log.player.error("IINA 未安装")
                        onResult(.iinaNotFound)
                    case .failure(let m):
                        self.logs = "❌ 启动失败：\(m)"
                        Log.player.error("IINA 启动失败: \(m)")
                        onResult(.failure(m))
                    }
                }
            }
        }
    }

    /// 停止内置播放器并释放资源。
    func stopBuiltinPlayer() {
        playTask?.cancel()
        playTask = nil
        danmakuTask?.cancel()
        danmakuTask = nil
        danmakuClient?.disconnect()
        danmakuClient = nil
        builtinPlayer?.stop()
        builtinPlayer = nil
    }

    /// 先解析真实房间号与 WBI token，再建立 BiliLive 弹幕 WebSocket。
    private func connectDanmaku(roomId: String) {
        guard room.site == .biliLive else {
            danmakuTask?.cancel()
            danmakuClient?.disconnect()
            danmakuClient = nil
            return
        }
        let rid = roomId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rid.isEmpty else { return }
        danmakuTask?.cancel()
        danmakuClient?.disconnect()
        danmakuClient = nil
        danmakuTask = Task { [weak self] in
            guard let self else { return }
            do {
                let connection = try await self.liveKit.resolveDanmakuConnection(
                    site: .biliLive,
                    roomId: rid
                )
                guard !Task.isCancelled, self.builtinPlayer != nil else { return }
                let client = DanmakuClient(connection: connection)
                self.danmakuClient = client
                client.connect()
                Log.network.debug("danmaku: 正在连接真实房间 \(connection.roomId)")
            } catch is CancellationError {
                return
            } catch {
                guard !Task.isCancelled else { return }
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
              let variant = variants.first(where: { $0.id == id }),
              let url = variant.url else { return nil }
        return url
    }
}
