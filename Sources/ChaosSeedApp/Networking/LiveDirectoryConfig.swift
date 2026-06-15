import Foundation

/// 直播目录 + 直播源解析的 API 端点配置。
///
/// 合并对齐 Rust `chaos_core::live_directory::client::LiveDirectoryEndpoints`
/// 与 `chaos_core::livestream::client::Endpoints`。
public struct LiveEndpoints: Sendable {
    public var biliLiveApiBase: String   // api.live.bilibili.com
    public var biliApiBase: String       // api.bilibili.com
    public var biliLiveBase: String      // live.bilibili.com
    public var huyaBase: String          // www.huya.com
    public var huyaMpBase: String        // mp.huya.com
    public var huyaLiveCdnBase: String   // live.cdn.huya.com
    public var huyaSearchBase: String    // search.cdn.huya.com
    public var douyuBase: String         // www.douyu.com
    public var douyuMBase: String        // m.douyu.com
    public var douyuPlayBase: String     // playweb.douyucdn.cn
    public var douyuCdnScheme: String    // https
    public var douyuP2pScheme: String    // https

    public init(
        biliLiveApiBase: String = "https://api.live.bilibili.com",
        biliApiBase: String = "https://api.bilibili.com",
        biliLiveBase: String = "https://live.bilibili.com",
        huyaBase: String = "https://www.huya.com",
        huyaMpBase: String = "https://mp.huya.com",
        huyaLiveCdnBase: String = "https://live.cdn.huya.com",
        huyaSearchBase: String = "https://search.cdn.huya.com",
        douyuBase: String = "https://www.douyu.com",
        douyuMBase: String = "https://m.douyu.com",
        douyuPlayBase: String = "https://playweb.douyucdn.cn",
        douyuCdnScheme: String = "https",
        douyuP2pScheme: String = "https"
    ) {
        self.biliLiveApiBase = biliLiveApiBase
        self.biliApiBase = biliApiBase
        self.biliLiveBase = biliLiveBase
        self.huyaBase = huyaBase
        self.huyaMpBase = huyaMpBase
        self.huyaLiveCdnBase = huyaLiveCdnBase
        self.huyaSearchBase = huyaSearchBase
        self.douyuBase = douyuBase
        self.douyuMBase = douyuMBase
        self.douyuPlayBase = douyuPlayBase
        self.douyuCdnScheme = douyuCdnScheme
        self.douyuP2pScheme = douyuP2pScheme
    }
}

/// 环境配置：当前时间（毫秒/秒）+ 种子 RNG（对齐 Rust `EnvConfig`）。
public final class EnvConfig: @unchecked Sendable {
    public let nowMs: () -> Int64
    public let nowS: () -> Int64
    private let lock = NSLock()
    private var rngState: UInt64

    public init(nowMs: @escaping () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) },
                nowS: @escaping () -> Int64 = { Int64(Date().timeIntervalSince1970) },
                seed: UInt64 = UInt64(Date().timeIntervalSince1970 * 1e9)) {
        self.nowMs = nowMs
        self.nowS = nowS
        self.rngState = seed == 0 ? 0x9E3779B97F4A7C15 : seed
    }

    /// 用 xorshift64 生成一个伪随机 UInt64（对齐 Rust fastrand 的用法）。
    public func nextUInt64() -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        var x = rngState
        x ^= x << 13
        x ^= x >> 7
        x ^= x << 17
        rngState = x == 0 ? 0x9E3779B97F4A7C15 : x
        return x
    }

    /// 对齐 Rust `EnvConfig::huya_uid`：
    /// `(time_ms % 1e10 * 1e3 + rand(100..1000)) % 4294967295`。
    public func huyaUid() -> UInt32 {
        let t = nowMs()
        let base = (t % 10_000_000_000) * 1000
        let r = Int64(nextUInt64() % 900) + 100 // [100, 999]
        let v = (base + r) % 4_294_967_295
        return UInt32(v)
    }

    /// 对齐 Rust `EnvConfig::douyu_did`：`md5(random_u64.to_string())`。
    public func douyuDid() -> String {
        let n = nextUInt64()
        return Crypto.md5Hex(String(n))
    }
}
