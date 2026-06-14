import Foundation
import os

/// 统一日志门面（基于 `os.Logger`）。
///
/// 用法：
/// ```swift
/// Log.network.debug("get_info 200 rid=\(rid)")
/// Log.parsing.error("missing room_id", error: e)
/// ```
///
/// **开关**：受 `UserDefaults` 的 `debugLogging` 控制。
/// - 关闭（默认）：只输出 `error`/`fault`（错误总会被记录）。
/// - 开启：额外输出 `debug`/`info`（HTTP 请求、解析过程、签名步骤等）。
///
/// 查看方式：
/// - `swift run` 运行时 → 直接看终端输出（os.Logger 默认打印 notice/error）。
/// - Xcode 运行时 → 控制台面板 + 断点。
/// - Console.app → 按 subsystem `com.zerodevi1.chaosseed` 过滤。
public enum Log {
    private static let subsystem = "com.zerodevi1.chaosseed"

    /// 网络层：HTTP 请求/响应、WBI 签名、buvid 获取。
    public static let network = AppLogger(category: "network")
    /// 解析层：JSON pointer 取值、manifest 组装、InputParser。
    public static let parsing = AppLogger(category: "parsing")
    /// 目录浏览：分类/推荐/搜索。
    public static let directory = AppLogger(category: "directory")
    /// 播放器：IINA 启动、Process。
    public static let player = AppLogger(category: "player")
    /// 应用层：导航、状态、Toast。
    public static let app = AppLogger(category: "app")

    /// 包装 `os.Logger`，把 `debugLogging` 开关下放到日志级别控制。
    public struct AppLogger {
        private let logger: Logger
        /// 是否输出 debug 级日志。
        /// - `debugLogging`（UserDefaults）为 true，或
        /// - 编译期标志 `FORCE_LOG`（用于 smoke 测试 / 手动调试强制开启）。
        private var verbose: Bool {
            #if FORCE_LOG
            return true
            #else
            return UserDefaults.standard.bool(forKey: "debugLogging")
            #endif
        }

        init(category: String) {
            self.logger = Logger(subsystem: subsystem, category: category)
        }

        public func debug(_ message: String) {
            guard verbose else { return }
            logger.debug("\(message, privacy: .public)")
        }
        public func info(_ message: String) {
            guard verbose else { return }
            logger.info("\(message, privacy: .public)")
        }
        /// notice 级别：默认始终输出（不受开关影响），用于关键节点。
        public func notice(_ message: String) {
            logger.notice("\(message, privacy: .public)")
        }
        public func error(_ message: String, error: Error? = nil) {
            if let error {
                logger.error("\(message): \(error.localizedDescription, privacy: .public)")
            } else {
                logger.error("\(message, privacy: .public)")
            }
        }
    }
}
