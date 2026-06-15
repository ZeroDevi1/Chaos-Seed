import SwiftUI

/// 统一播放器协议：IINA 外部播放器与内置 AVPlayer 播放器实现同一接口。
///
/// - `playerView`：返回内置播放器的 SwiftUI 视图；外部播放器（如 IINA）返回 nil。
/// - `play(url:hints:)`：启动播放；外部播放器通过系统调用拉起，内置播放器设置 `playerView` 后再调用。
///
/// 所有实现类都是 @MainActor，协议标注 @MainActor 避免 Swift 6 并发警告。
@MainActor
public protocol LivePlayer {
    /// 内置播放器的 SwiftUI 视图；外部播放器（如 IINA）返回 nil。
    var playerView: AnyView? { get }
    /// 构造播放所需的 HTTP header（Referer + User-Agent），Bili CDN 需要这两项才能正常拉流。
    func play(url: URL, hints: PlaybackHints)
}

/// 可播放直播流的内置播放器实现类型（供 DetailView 判断是否使用内置播放器）。
@MainActor
public protocol BuiltinPlayable: LivePlayer {
    /// 播放器是否正在播放。
    var isPlaying: Bool { get }
    /// 停止播放并清理资源。
    func stop()
}
