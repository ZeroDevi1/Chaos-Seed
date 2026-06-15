import AppKit
import SwiftUI

struct DanmakuRenderItem: Identifiable {
    let id: UUID
    let text: String
    let imageURL: URL?
    let frame: CGRect
    let colorRGB: UInt32
    let opacity: Double
}

enum DanmakuLayoutEngine {
    private struct PreparedComment {
        var comment: DanmakuComment
        var repeatCount: Int

        var displayText: String {
            guard repeatCount > 1 else { return comment.text }
            return "\(comment.text)  ×\(repeatCount)"
        }
    }

    static func layout(
        comments: [DanmakuComment],
        config: DanmakuConfig,
        size: CGSize,
        now: Date
    ) -> [DanmakuRenderItem] {
        guard size.width > 0, size.height > 0 else { return [] }
        let prepared = prepare(comments: comments, config: config)
        switch config.mode {
        case .scroll:
            return scrolling(prepared, config: config, size: size, now: now)
        case .top, .bottom:
            return fixed(prepared, config: config, size: size, now: now)
        }
    }

    static func isBlocked(_ comment: DanmakuComment, rules: [String]) -> Bool {
        let candidate = comment.text
        for rawRule in rules {
            let rule = rawRule.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !rule.isEmpty else { continue }
            if rule.count > 2, rule.hasPrefix("/"), rule.hasSuffix("/") {
                let pattern = String(rule.dropFirst().dropLast())
                if let expression = try? NSRegularExpression(
                    pattern: pattern,
                    options: [.caseInsensitive]
                ), expression.firstMatch(
                    in: candidate,
                    range: NSRange(candidate.startIndex..., in: candidate)
                ) != nil {
                    return true
                }
            } else if candidate.localizedCaseInsensitiveContains(rule) {
                return true
            }
        }
        return false
    }

    static func isValidBlockRule(_ rule: String) -> Bool {
        let trimmed = rule.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        guard trimmed.count > 2, trimmed.hasPrefix("/"), trimmed.hasSuffix("/") else {
            return true
        }
        return (try? NSRegularExpression(pattern: String(trimmed.dropFirst().dropLast()))) != nil
    }

    private static func prepare(
        comments: [DanmakuComment],
        config: DanmakuConfig
    ) -> [PreparedComment] {
        let filtered = comments
            .filter { $0.opacity >= config.minOpacity }
            .filter { !isBlocked($0, rules: config.blockedWords) }
            .sorted { $0.receivedAt < $1.receivedAt }
        guard config.collapseDuplicates else {
            return filtered.map { PreparedComment(comment: $0, repeatCount: 1) }
        }

        var result: [PreparedComment] = []
        var lastIndexByKey: [String: Int] = [:]
        for comment in filtered {
            let key = duplicateKey(comment)
            if let index = lastIndexByKey[key],
               comment.receivedAt.timeIntervalSince(result[index].comment.receivedAt) <= 3 {
                result[index].repeatCount += 1
                continue
            }
            lastIndexByKey[key] = result.count
            result.append(PreparedComment(comment: comment, repeatCount: 1))
        }
        return result
    }

    private static func duplicateKey(_ comment: DanmakuComment) -> String {
        if let imageUrl = comment.imageUrl {
            return "image:\(imageUrl)"
        }
        return "text:\(comment.text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())"
    }

    private static func scrolling(
        _ comments: [PreparedComment],
        config: DanmakuConfig,
        size: CGSize,
        now: Date
    ) -> [DanmakuRenderItem] {
        let lineHeight = max(config.fontSize + 8, 40)
        let availableHeight = max(lineHeight, size.height * config.displayAreaRatio - 12)
        let laneCount = max(1, Int(availableHeight / lineHeight))
        let speed = CGFloat(max(35, 90 * config.speed))
        var laneReadyAt = Array(repeating: Date.distantPast, count: laneCount)
        var result: [DanmakuRenderItem] = []

        for prepared in comments.suffix(160) {
            let start = prepared.comment.receivedAt
            guard start <= now else { continue }
            let width = itemWidth(prepared, fontSize: config.fontSize)
            let lifetime = TimeInterval((size.width + width) / speed)
            let age = now.timeIntervalSince(start)
            guard age <= lifetime else { continue }
            guard let lane = laneReadyAt.firstIndex(where: { start >= $0 }) else {
                continue
            }

            // 同轨上一条的尾部进入画面并留出间隔后，才允许下一条发车。
            laneReadyAt[lane] = start.addingTimeInterval(TimeInterval((width + 24) / speed))
            let x = size.width - CGFloat(age) * speed
            let y = 8 + CGFloat(lane) * lineHeight
            result.append(renderItem(
                prepared,
                frame: CGRect(x: x, y: y, width: width, height: lineHeight),
                config: config,
                opacityMultiplier: 1
            ))
        }
        return Array(result.suffix(80))
    }

    private static func fixed(
        _ comments: [PreparedComment],
        config: DanmakuConfig,
        size: CGSize,
        now: Date
    ) -> [DanmakuRenderItem] {
        let lineHeight = max(config.fontSize + 8, 40)
        let availableHeight = max(lineHeight, size.height * config.displayAreaRatio - 12)
        let laneCount = max(1, Int(availableHeight / lineHeight))
        let lifetime: TimeInterval = 5
        var laneReadyAt = Array(repeating: Date.distantPast, count: laneCount)
        var result: [DanmakuRenderItem] = []

        for prepared in comments.suffix(120) {
            let start = prepared.comment.receivedAt
            let age = now.timeIntervalSince(start)
            guard age >= 0, age <= lifetime else { continue }
            guard let lane = laneReadyAt.firstIndex(where: { start >= $0 }) else {
                continue
            }
            laneReadyAt[lane] = start.addingTimeInterval(lifetime)
            let width = itemWidth(prepared, fontSize: config.fontSize)
            let x = max(8, (size.width - width) / 2)
            let y: CGFloat
            if config.mode == .top {
                y = 8 + CGFloat(lane) * lineHeight
            } else {
                y = size.height - 8 - CGFloat(lane + 1) * lineHeight
            }
            let fade = age > lifetime - 0.7 ? max(0, (lifetime - age) / 0.7) : 1
            result.append(renderItem(
                prepared,
                frame: CGRect(x: x, y: y, width: width, height: lineHeight),
                config: config,
                opacityMultiplier: fade
            ))
        }
        return result
    }

    private static func itemWidth(_ prepared: PreparedComment, fontSize: CGFloat) -> CGFloat {
        if prepared.comment.isEmoticon {
            return CGFloat(prepared.comment.imageWidth ?? 64)
        }
        let font = NSFont.systemFont(ofSize: fontSize, weight: .semibold)
        let measured = (prepared.displayText as NSString).size(withAttributes: [.font: font]).width
        return min(720, max(24, measured + 12))
    }

    private static func renderItem(
        _ prepared: PreparedComment,
        frame: CGRect,
        config: DanmakuConfig,
        opacityMultiplier: Double
    ) -> DanmakuRenderItem {
        DanmakuRenderItem(
            id: prepared.comment.id,
            text: prepared.displayText,
            imageURL: prepared.comment.imageUrl.flatMap(URL.init(string:)),
            frame: frame,
            colorRGB: config.showColored ? prepared.comment.colorRGB : 0xFF_FF_FF,
            opacity: min(1, max(0, config.opacity * prepared.comment.opacity * opacityMultiplier))
        )
    }
}

/// 到达时间驱动的弹幕叠加层。文本走 Canvas，表情使用 AsyncImage，二者共享轨道布局。
public struct DanmakuOverlay: View {
    let comments: [DanmakuComment]
    let config: DanmakuConfig
    let size: CGSize
    let isPlaying: Bool

    public init(
        comments: [DanmakuComment],
        config: DanmakuConfig,
        size: CGSize,
        isPlaying: Bool = true
    ) {
        self.comments = comments
        self.config = config
        self.size = size
        self.isPlaying = isPlaying
    }

    public var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !isPlaying)) { context in
            let items = DanmakuLayoutEngine.layout(
                comments: comments,
                config: config,
                size: size,
                now: context.date
            )
            ZStack(alignment: .topLeading) {
                Canvas { graphics, _ in
                    for item in items where item.imageURL == nil && !item.text.isEmpty {
                        drawText(item, in: &graphics)
                    }
                }

                ForEach(items.filter { $0.imageURL != nil }) { item in
                    AsyncImage(url: item.imageURL) { phase in
                        if let image = phase.image {
                            image.resizable().scaledToFit()
                        }
                    }
                    .frame(width: item.frame.width, height: item.frame.height)
                    .opacity(item.opacity)
                    .position(x: item.frame.midX, y: item.frame.midY)
                }
            }
        }
        .clipped()
        .accessibilityHidden(true)
    }

    private func drawText(_ item: DanmakuRenderItem, in graphics: inout GraphicsContext) {
        let origin = CGPoint(x: item.frame.minX + 6, y: item.frame.midY)
        let shadow = Text(item.text)
            .font(.system(size: config.fontSize, weight: .semibold))
            .foregroundStyle(.black.opacity(item.opacity * 0.9))
        for offset in [
            CGSize(width: -1, height: 0),
            CGSize(width: 1, height: 0),
            CGSize(width: 0, height: -1),
            CGSize(width: 0, height: 1),
        ] {
            graphics.draw(
                shadow,
                at: CGPoint(x: origin.x + offset.width, y: origin.y + offset.height),
                anchor: .leading
            )
        }
        graphics.draw(
            Text(item.text)
                .font(.system(size: config.fontSize, weight: .semibold))
                .foregroundStyle(color(item.colorRGB).opacity(item.opacity)),
            at: origin,
            anchor: .leading
        )
    }

    private func color(_ rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }
}
