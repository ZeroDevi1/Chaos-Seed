import SwiftUI

/// 主题与平台配色，对齐原型 `styles.css` 的平台徽章色。
public enum Theme {
    /// 平台徽章背景色（对齐 `.platform-badge.bili/.douyu/.huya`）。
    static func badgeColor(for site: Site) -> Color {
        switch site {
        case .biliLive: return Color(red: 0x00/255, green: 0xa1/255, blue: 0xd6/255) // #00a1d6
        case .douyu: return Color(red: 0xff/255, green: 0x5d/255, blue: 0x23/255)     // #ff5d23
        case .huya: return Color(red: 0x3c/255, green: 0xc0/255, blue: 0x3c/255)      // #3cc03c
        }
    }
}

/// 在线人数格式化（对齐原型 `formatViewers`，万级显示一位小数）。
public func formatViewers(_ n: Int) -> String {
    if n >= 10_000 {
        return String(format: "%.1f万", Double(n) / 10_000)
    }
    return n.formatted()
}
