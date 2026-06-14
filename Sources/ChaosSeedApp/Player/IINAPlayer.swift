import AppKit
import Foundation

/// IINA 播放器桥。
///
/// 设计文档 §4.5：
/// - 优先用 `NSWorkspace.shared.open(_:)` 打开流 URL；
/// - 若 `PlaybackHints` 含 Referer/UA/Cookie，改用 IINA CLI（`Process`）传入 `--mpv-http-header-fields`。
///
/// v1：保留骨架与日志，实际拉起留到真实网络层接入后启用。
public enum IINAPlayer {
    public enum LaunchResult {
        case launched
        case iinaNotFound
        case failure(String)
    }

    /// 默认 IINA 应用路径。
    public static let defaultAppPath = "/Applications/IINA.app"
    /// IINA CLI 二进制路径。
    public static let defaultCLIPath = "/Applications/IINA.app/Contents/MacOS/iina-cli"

    /// 启动 IINA 播放指定流。
    ///
    /// - Parameters:
    ///   - url: 直播流直连 URL。
    ///   - hints: 来自 `LiveManifest.playback` 的 HTTP header 提示。
    ///   - appPath: IINA.app 路径，默认 `/Applications/IINA.app`。
    public static func launch(url: String, hints: PlaybackHints, appPath: String = defaultAppPath) -> LaunchResult {
        guard FileManager.default.fileExists(atPath: appPath) else {
            return .iinaNotFound
        }
        let needsHeaders = hints.referer != nil || hints.userAgent != nil
        if !needsHeaders, let streamURL = URL(string: url) {
            // 简单情况：直接让系统用 IINA 打开 URL。
            let config = NSWorkspace.OpenConfiguration()
            return open(url: streamURL, appPath: appPath, config: config)
        }
        // 需要 header：调用 iina-cli 传入 `--mpv-http-header-fields`。
        return launchCLI(url: url, hints: hints)
    }

    private static func open(url: URL, appPath: String, config: NSWorkspace.OpenConfiguration) -> LaunchResult {
        guard let appURL = URL(string: "file://\(appPath)") else {
            return .failure("invalid app path: \(appPath)")
        }
        // `open(_:withApplicationAt:configuration:)` 不抛出——通过闭包回报失败。
        var launchError: Error?
        NSWorkspace.shared.open([url], withApplicationAt: appURL, configuration: config) { _, error in
            launchError = error
        }
        if let launchError {
            return .failure(launchError.localizedDescription)
        }
        return .launched
    }

    private static func launchCLI(url: String, hints: PlaybackHints) -> LaunchResult {
        guard FileManager.default.isExecutableFile(atPath: defaultCLIPath) else {
            // CLI 不存在时退回直接打开 URL（header 会丢失）。
            if let streamURL = URL(string: url) {
                _ = NSWorkspace.shared.open(streamURL)
                return .launched
            }
            return .failure("iina-cli not found")
        }
        let task = Process()
        task.executableURL = URL(fileURLWithPath: defaultCLIPath)
        var args: [String] = [url]
        if let referer = hints.referer {
            args.append(contentsOf: ["--mpv-http-header-fields", "Referer: \(referer)"])
        }
        if let ua = hints.userAgent {
            args.append(contentsOf: ["--mpv-http-header-fields", "User-Agent: \(ua)"])
        }
        task.arguments = args
        do {
            try task.run()
            return .launched
        } catch {
            return .failure(error.localizedDescription)
        }
    }
}
