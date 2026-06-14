import SwiftUI

/// 详情页视图模型：解析 manifest，管理清晰度选中态与 IINA 启动。
@MainActor
public final class DetailViewModel: ObservableObject {
    @Published var variants: [StreamVariant] = []
    @Published var selectedVariantId: String?
    @Published var loading = false
    @Published var logs: String = ""

    let room: LiveRoomCard
    private let liveKit: LiveKit
    private var loadTask: Task<Void, Never>?

    public init(room: LiveRoomCard, liveKit: LiveKit) {
        self.room = room
        self.liveKit = liveKit
    }

    func decode() {
        loadTask?.cancel()
        loading = true
        selectedVariantId = nil
        logs = "正在解析直播间信息…"
        loadTask = Task { [weak self] in
            guard let self else { return }
            do {
                let manifest = try await self.liveKit.decodeManifest(input: self.room.input, options: .default)
                guard !Task.isCancelled else { return }
                self.variants = manifest.variants
                self.logs = "已解析 \(manifest.variants.count) 个清晰度/线路组合。"
            } catch {
                guard !Task.isCancelled else { return }
                self.logs = "解析失败：\(error.localizedDescription)"
            }
            self.loading = false
        }
    }

    func play(onResult: @escaping (IINAPlayer.LaunchResult) -> Void) {
        guard let id = selectedVariantId,
              let variant = variants.first(where: { $0.id == id }) else { return }
        logs = "正在解析「\(variant.label)」的播放地址…"

        let needResolve = !variant.isResolved
        let site = room.site
        let roomId = room.roomId
        Task { [weak self] in
            guard let self else { return }
            var current = variant
            if needResolve {
                do {
                    current = try await self.liveKit.resolveVariant(site: site, roomId: roomId, variantId: id)
                } catch {
                    await MainActor.run {
                        self.logs = "解析失败：\(error.localizedDescription)"
                        onResult(.failure(error.localizedDescription))
                    }
                    return
                }
            }
            guard let url = current.url else {
                await MainActor.run {
                    self.logs = "未能获取播放地址"
                    onResult(.failure("no url"))
                }
                return
            }
            let result = IINAPlayer.launch(
                url: url,
                hints: PlaybackHints(referer: site == .biliLive ? "https://live.bilibili.com/" : nil)
            )
            await MainActor.run {
                switch result {
                case .launched:
                    self.logs = "IINA 已启动：\(self.room.title) / \(current.label)"
                case .iinaNotFound:
                    self.logs = "未检测到 IINA，请安装后重试。"
                case .failure(let m):
                    self.logs = "启动失败：\(m)"
                }
                onResult(result)
            }
        }
    }

    func copySelectedUrl() -> String? {
        guard let id = selectedVariantId,
              let variant = variants.first(where: { $0.id == id }),
              let url = variant.url else { return nil }
        return url
    }
}
