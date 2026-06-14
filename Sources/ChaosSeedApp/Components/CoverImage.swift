import SwiftUI

/// 直播间封面图：优先用真实 URL 加载，加载失败/无 URL 时显示渐变占位 + 平台色块 + 图标。
///
/// 用法：`CoverImage(cover: room.cover, site: room.site)`
public struct CoverImage: View {
    let cover: String?
    let site: Site?
    let placeholderIcon: String

    public init(cover: String?, site: Site? = nil, placeholderIcon: String = "play.tv") {
        self.cover = cover
        self.site = site
        self.placeholderIcon = placeholderIcon
    }

    public var body: some View {
        GeometryReader { geo in
            if let cover, let url = URL(string: cover), !cover.isEmpty {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .empty:
                        placeholder.frame(width: geo.size.width, height: geo.size.height)
                    case .success(let image):
                        image
                            .resizable()
                            .scaledToFill()
                            .frame(width: geo.size.width, height: geo.size.height)
                            .clipped()
                    case .failure:
                        placeholder.frame(width: geo.size.width, height: geo.size.height)
                    @unknown default:
                        placeholder.frame(width: geo.size.width, height: geo.size.height)
                    }
                }
            } else {
                placeholder.frame(width: geo.size.width, height: geo.size.height)
            }
        }
    }

    /// 渐变占位 + 中心图标 + 平台色角标。
    private var placeholder: some View {
        ZStack {
            LinearGradient(
                colors: placeholderColors,
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            VStack(spacing: 8) {
                Image(systemName: placeholderIcon)
                    .font(.system(size: 30, weight: .light))
                    .foregroundStyle(.white.opacity(0.55))
                if let site {
                    Text(site.displayName)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.4))
                }
            }
        }
    }

    /// 根据平台选不同色调的占位渐变，让卡片视觉上有区分。
    private var placeholderColors: [Color] {
        switch site {
        case .biliLive:
            return [Color(red: 0/255, green: 0xa1/255, blue: 0xd6/255).opacity(0.85),
                    Color(red: 0/255, green: 0x4a/255, blue: 0x7a/255).opacity(0.9)]
        case .douyu:
            return [Color(red: 0xff/255, green: 0x5d/255, blue: 0x23/255).opacity(0.85),
                    Color(red: 0xa3/255, green: 0x2a/255, blue: 0x10/255).opacity(0.9)]
        case .huya:
            return [Color(red: 0x3c/255, green: 0xc0/255, blue: 0x3c/255).opacity(0.85),
                    Color(red: 0x1f/255, green: 0x7a/255, blue: 0x2a/255).opacity(0.9)]
        case .none:
            return [Color.gray.opacity(0.35), Color.gray.opacity(0.5)]
        }
    }
}
