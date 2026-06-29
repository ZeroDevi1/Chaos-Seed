import Foundation
import Compression

// MARK: - 数据结构

/// 弹幕评论（文本或表情）。
public struct DanmakuComment: Sendable, Identifiable, Equatable {
    public let id: UUID
    /// 弹幕文本内容；表情弹幕为空。
    public let text: String
    /// 发送者昵称。
    public let user: String
    /// 表情图片 URL（nil 表示纯文本弹幕）。
    public let imageUrl: String?
    /// 表情图片宽度（用于 Canvas 缩放）。
    public let imageWidth: Int?
    /// 本机收到该条弹幕的时间，渲染器据此计算位置与过期时间。
    public let receivedAt: Date
    /// Bilibili 载荷中的 RGB 颜色。
    public let colorRGB: UInt32
    /// 载荷透明度；旧协议通常为 1。
    public let opacity: Double
    /// 来源建议的显示模式。
    public let sourceMode: DanmakuMode

    public init(
        id: UUID = UUID(),
        text: String,
        user: String = "",
        imageUrl: String? = nil,
        imageWidth: Int? = nil,
        receivedAt: Date = Date(),
        colorRGB: UInt32 = 0xFF_FF_FF,
        opacity: Double = 1,
        sourceMode: DanmakuMode = .scroll
    ) {
        self.id = id
        self.text = text
        self.user = user
        self.imageUrl = imageUrl
        self.imageWidth = imageWidth
        self.receivedAt = receivedAt
        self.colorRGB = colorRGB
        self.opacity = opacity
        self.sourceMode = sourceMode
    }

    /// 是否为表情弹幕。
    public var isEmoticon: Bool { imageUrl != nil }
}

// MARK: - 弹幕配置

/// 弹幕显示配置（持久化到 @AppStorage）。
public struct DanmakuConfig: Equatable, Codable {
    /// 字号（8-48）。
    public var fontSize: CGFloat = 18
    /// 不透明度（0.1-1.0）。
    public var opacity: Double = 0.85
    /// 滚动速度因子（0.5-3.0），值越大越快。
    public var speed: Double = 1.0
    /// 显示区域：0.25=顶部1/4 / 0.5=半屏 / 1.0=全屏。
    public var displayAreaRatio: Double = 0.5
    /// 显示模式。
    ///
    /// 仅用于兼容旧版持久化配置；新版渲染按每条弹幕自身的 sourceMode 分类。
    public var mode: DanmakuMode = .scroll
    /// 是否显示滚动弹幕。
    public var showScrolling = true
    /// 是否显示顶部固定弹幕。
    public var showTop = true
    /// 是否显示底部固定弹幕。
    public var showBottom = true
    /// 是否显示彩色弹幕（非彩色弹幕显示为白色）。
    public var showColored: Bool = true
    /// 屏蔽词列表。
    public var blockedWords: [String] = []
    /// 透明度低于此值的弹幕不显示。
    public var minOpacity: Double = 0.0
    /// 短时间内相同内容折叠为一条，并显示重复次数。
    public var collapseDuplicates: Bool = true

    public static let `default` = DanmakuConfig()

    private enum CodingKeys: String, CodingKey {
        case fontSize
        case opacity
        case speed
        case displayAreaRatio
        case mode
        case showScrolling
        case showTop
        case showBottom
        case showColored
        case blockedWords
        case minOpacity
        case collapseDuplicates
    }

    public init() {}

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        fontSize = CGFloat(try values.decodeIfPresent(Double.self, forKey: .fontSize) ?? 18)
        opacity = try values.decodeIfPresent(Double.self, forKey: .opacity) ?? 0.85
        speed = try values.decodeIfPresent(Double.self, forKey: .speed) ?? 1
        displayAreaRatio = try values.decodeIfPresent(Double.self, forKey: .displayAreaRatio) ?? 0.5
        mode = try values.decodeIfPresent(DanmakuMode.self, forKey: .mode) ?? .scroll
        showScrolling = try values.decodeIfPresent(Bool.self, forKey: .showScrolling) ?? true
        showTop = try values.decodeIfPresent(Bool.self, forKey: .showTop) ?? true
        showBottom = try values.decodeIfPresent(Bool.self, forKey: .showBottom) ?? true
        showColored = try values.decodeIfPresent(Bool.self, forKey: .showColored) ?? true
        blockedWords = try values.decodeIfPresent([String].self, forKey: .blockedWords) ?? []
        minOpacity = try values.decodeIfPresent(Double.self, forKey: .minOpacity) ?? 0
        collapseDuplicates = try values.decodeIfPresent(Bool.self, forKey: .collapseDuplicates) ?? true
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(Double(fontSize), forKey: .fontSize)
        try values.encode(opacity, forKey: .opacity)
        try values.encode(speed, forKey: .speed)
        try values.encode(displayAreaRatio, forKey: .displayAreaRatio)
        try values.encode(mode, forKey: .mode)
        try values.encode(showScrolling, forKey: .showScrolling)
        try values.encode(showTop, forKey: .showTop)
        try values.encode(showBottom, forKey: .showBottom)
        try values.encode(showColored, forKey: .showColored)
        try values.encode(blockedWords, forKey: .blockedWords)
        try values.encode(minOpacity, forKey: .minOpacity)
        try values.encode(collapseDuplicates, forKey: .collapseDuplicates)
    }
}

/// 弹幕显示模式。
public enum DanmakuMode: String, CaseIterable, Codable, Sendable {
    case scroll  // 从右往左滚动
    case top     // 顶部固定
    case bottom  // 底部固定

    public var label: String {
        switch self {
        case .scroll: return "滚动"
        case .top: return "顶部"
        case .bottom: return "底部"
        }
    }
}

// MARK: - 弹幕客户端

/// BiliLive / Douyu / Huya 统一弹幕 WebSocket 客户端。
///
/// 协议对齐 `main` 分支 `chaos-core/src/danmaku/platforms`。
@MainActor
public final class DanmakuClient: ObservableObject {
    /// 收到的弹幕流（供 UI 渲染）。
    @Published public var comments: [DanmakuComment] = []
    /// 连接状态。
    @Published public private(set) var isConnected = false
    @Published public private(set) var isReconnecting = false
    @Published public private(set) var error: String?
    /// 最近一次接收统计，便于诊断协议版本变化。
    @Published public private(set) var receivedPacketCount = 0
    @Published public private(set) var jsonMessageCount = 0
    @Published public private(set) var lastPacketSummary = "-"
    @Published public private(set) var lastJSONSummary = "-"
    @Published public private(set) var packetHistogram: [String: Int] = [:]
    @Published public private(set) var lastTextDiagnostic = "-"
    @Published public private(set) var lastInflateDiagnostic = "-"

    private let connection: DanmakuConnectionInfo
    private var webSocketTask: URLSessionWebSocketTask?
    private var heartbeatTask: Task<Void, Never>?
    private var receiveTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var shouldReconnect = false
    private var reconnectAttempt = 0
    private var sequence: UInt32 = 1
    private let maxComments = 400

    /// - Parameters:
    ///   - roomId: 直播间真实 room_id（长号）。
    ///   - token: 弹幕 WebSocket 认证 token（从 getDanmuInfo API 获取）。
    public init(connection: DanmakuConnectionInfo) {
        self.connection = connection
    }

    /// 测试与兼容入口；正式播放应使用 `BiliDanmakuResolver` 提供完整凭据。
    public convenience init(roomId: String, token: String) {
        self.init(connection: DanmakuConnectionInfo(
            roomId: roomId,
            uid: 0,
            token: token,
            buvid: Self.makeFallbackBuvid(),
            endpoint: URL(string: "wss://broadcastlv.chat.bilibili.com/sub")!,
            emoticons: [:]
        ))
    }

    /// 连接弹幕服务器并开始接收。
    public func connect() {
        shouldReconnect = true
        reconnectAttempt = 0
        openConnection()
    }

    private func openConnection() {
        guard shouldReconnect else { return }
        guard webSocketTask == nil else { return }
        var request = URLRequest(url: connection.endpoint)
        request.timeoutInterval = 15
        if connection.site == .douyu {
            request.setValue("https://www.douyu.com", forHTTPHeaderField: "Origin")
            request.setValue("chaos-seed/0.1 (douyu-danmaku)", forHTTPHeaderField: "User-Agent")
        }
        webSocketTask = URLSession.shared.webSocketTask(with: request)
        webSocketTask?.resume()

        joinRoom()
        startReceive()
        error = nil
    }

    /// 断开连接。
    public func disconnect() {
        shouldReconnect = false
        reconnectTask?.cancel()
        reconnectTask = nil
        heartbeatTask?.cancel()
        heartbeatTask = nil
        receiveTask?.cancel()
        receiveTask = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        isConnected = false
        isReconnecting = false
        reconnectAttempt = 0
    }

    // MARK: - 进房认证

    private func joinRoom() {
        switch connection.site {
        case .biliLive:
            joinBiliRoom()
        case .douyu:
            sendPlatformPacket(Self.encodeDouyuPacket("type@=loginreq/roomid@=\(connection.roomId)/"))
            sendPlatformPacket(Self.encodeDouyuPacket("type@=joingroup/rid@=\(connection.roomId)/gid@=-9999/"))
        case .huya:
            guard let yyuid = connection.huyaYyuid, let uid = connection.huyaUid else {
                error = "huya danmaku credentials missing"
                return
            }
            sendPlatformPacket(Self.encodeHuyaJoin(yyuid: yyuid, uid: uid), markConnected: true)
        }
    }

    private func joinBiliRoom() {
        let auth: [String: Any] = [
            "uid": connection.uid,
            "roomid": UInt64(connection.roomId) ?? 0,
            "protover": 2,
            "buvid": connection.buvid,
            "platform": "web",
            "type": 2,
            "key": connection.token,
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: auth) else {
            error = "joinRoom payload encoding failed"
            return
        }
        let packet = encodePacket(body: body, operation: 7, protover: 1)
        webSocketTask?.send(.data(packet)) { [weak self] err in
            if let err {
                Task { @MainActor in
                    self?.handleConnectionFailure("joinRoom failed: \(err.localizedDescription)")
                }
            }
        }
    }

    private func sendPlatformPacket(_ packet: Data, markConnected: Bool = false) {
        webSocketTask?.send(.data(packet)) { [weak self] err in
            Task { @MainActor in
                guard let self else { return }
                if let err {
                    self.handleConnectionFailure("joinRoom failed: \(err.localizedDescription)")
                    return
                }
                if markConnected {
                    self.markPlatformConnected()
                }
            }
        }
    }

    private func markPlatformConnected() {
        guard !isConnected else { return }
        isConnected = true
        isReconnecting = false
        error = nil
        reconnectAttempt = 0
        startHeartbeat()
        Log.network.debug("danmaku: \(connection.site.rawKey) 进房 room=\(connection.roomId)")
    }

    // MARK: - 心跳

    private func startHeartbeat() {
        sendHeartbeat()
        heartbeatTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 30_000_000_000)
                if Task.isCancelled { break }
                self.sendHeartbeat()
            }
        }
    }

    private func sendHeartbeat() {
        let packet: Data
        switch connection.site {
        case .biliLive:
            packet = encodePacket(body: Data(), operation: 2, protover: 1)
        case .douyu:
            packet = Self.encodeDouyuPacket("type@=mrkl/")
        case .huya:
            packet = Data([0x00, 0x14, 0x1D, 0x00, 0x0C, 0x2C, 0x36, 0x00, 0x4C])
        }
        webSocketTask?.send(.data(packet)) { _ in }
    }

    // MARK: - 接收循环

    private func startReceive() {
        receiveTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                guard let task = self.webSocketTask else { break }
                do {
                    let msg = try await task.receive()
                    switch msg {
                    case .data(let data):
                        await self.handleFrame(data: data, depth: 0)
                    case .string(let text):
                        if self.connection.site == .douyu {
                            await self.handleDouyuText(text)
                        }
                    @unknown default:
                        break
                    }
                } catch {
                    guard !Task.isCancelled else { break }
                    self.handleConnectionFailure("receive error: \(error.localizedDescription)")
                    break
                }
            }
        }
    }

    private func handleConnectionFailure(_ message: String) {
        guard shouldReconnect, reconnectTask == nil else { return }
        isConnected = false
        heartbeatTask?.cancel()
        heartbeatTask = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        lastTextDiagnostic = message
        scheduleReconnect(after: message)
    }

    private func scheduleReconnect(after message: String) {
        guard shouldReconnect else { return }
        reconnectTask?.cancel()
        reconnectAttempt += 1
        let delay = Self.reconnectDelay(forAttempt: reconnectAttempt)
        isReconnecting = true
        error = "连接中断，\(Int(delay)) 秒后重连：\(message)"
        Log.network.error(
            "danmaku: \(connection.site.rawKey) 连接中断 room=\(connection.roomId)，"
                + "\(Int(delay)) 秒后重连：\(message)"
        )
        reconnectTask = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            } catch {
                return
            }
            guard let self, self.shouldReconnect else { return }
            self.reconnectTask = nil
            self.openConnection()
        }
    }

    static func reconnectDelay(forAttempt attempt: Int) -> TimeInterval {
        let exponent = min(max(attempt - 1, 0), 4)
        return min(pow(2, Double(exponent)), 15)
    }

    // MARK: - 帧处理

    private func handleFrame(data: Data, depth: Int) async {
        switch connection.site {
        case .biliLive:
            await handleBiliFrame(data: data, depth: depth)
        case .douyu:
            for text in Self.decodeDouyuPackets(data) {
                await handleDouyuText(text)
            }
        case .huya:
            await handleHuyaFrame(data)
        }
    }

    private func handleBiliFrame(data: Data, depth: Int) async {
        guard depth < 4 else { return }

        let packets = parsePackets(data: data)
        for p in packets {
            receivedPacketCount += 1
            lastPacketSummary = "op=\(p.operation), proto=\(p.protover), bytes=\(p.body.count)"
            packetHistogram["\(p.operation)/\(p.protover)", default: 0] += 1
            switch p.operation {
            case 8:
                if Self.isSuccessfulAuthResponse(p.body) {
                    isConnected = true
                    error = nil
                    startHeartbeat()
                    Log.network.debug("danmaku: 进房确认 room=\(connection.roomId)")
                } else {
                    isConnected = false
                    error = "danmaku authentication rejected"
                    disconnect()
                }
            case 5:
                switch p.protover {
                case 0, 1:
                    if let text = String(data: p.body, encoding: .utf8) {
                        let parts = text.components(separatedBy: "\u{0}").filter {
                            $0.trimmingCharacters(in: .whitespaces).hasPrefix("{")
                        }
                        lastTextDiagnostic = "chars=\(text.count), parts=\(parts.count), prefix=\(String(text.prefix(80)).debugDescription)"
                        for part in parts {
                            await handleJson(part)
                        }
                    }
                case 2:
                    if let inflated = Self.inflateBiliZlib(p.body) {
                        await handleInflatedFrame(inflated, depth: depth)
                    } else {
                        error = "danmaku zlib inflate failed"
                    }
                case 3:
                    if let inflated = Self.inflateBrotli(p.body) {
                        await handleInflatedFrame(inflated, depth: depth)
                    } else {
                        error = "danmaku brotli inflate failed"
                    }
                default:
                    break
                }
            default:
                break
            }
        }
    }

    private func handleDouyuText(_ text: String) async {
        receivedPacketCount += 1
        lastPacketSummary = "douyu chars=\(text.count)"
        lastTextDiagnostic = String(text.prefix(240))
        let eventType = text
            .split(separator: "/", maxSplits: 1)
            .first?
            .replacingOccurrences(of: "type@=", with: "") ?? "unknown"
        packetHistogram[eventType, default: 0] += 1
        if text.hasPrefix("type@=loginres") {
            markPlatformConnected()
            return
        }
        guard let comment = Self.parseDouyuComment(text, receivedAt: Date()) else { return }
        await append(comment)
    }

    private func handleHuyaFrame(_ data: Data) async {
        receivedPacketCount += 1
        lastPacketSummary = "huya bytes=\(data.count)"
        markPlatformConnected()
        do {
            guard try HuyaJCE.int32(data, tag: 0) == 7,
                  let push = try HuyaJCE.bytes(data, tag: 1),
                  try HuyaJCE.int64(push, tag: 1) == 1400,
                  let message = try HuyaJCE.bytes(push, tag: 2),
                  let text = try HuyaJCE.string(message, tag: 3),
                  !text.isEmpty else {
                return
            }
            let userInfo = try HuyaJCE.structBytes(message, tag: 0)
            let user = try userInfo.flatMap { try HuyaJCE.string($0, tag: 2) } ?? ""
            await append(DanmakuComment(text: text, user: user))
        } catch {
            self.error = "huya danmaku decode failed: \(error.localizedDescription)"
        }
    }

    private func handleInflatedFrame(_ data: Data, depth: Int) async {
        let prefix = String(data: data.prefix(80), encoding: .utf8)
        let firstBytes = data.prefix(16).map { String(format: "%02x", $0) }.joined()
        lastInflateDiagnostic =
            "bytes=\(data.count), hex=\(firstBytes), text=\((prefix ?? "-").debugDescription)"

        if Self.hasPacketHeader(data) {
            await handleFrame(data: data, depth: depth + 1)
            return
        }
        if let text = String(data: data, encoding: .utf8) {
            await handleJSONStream(text)
        }
    }

    // MARK: - JSON 解析

    private func handleJson(_ raw: String) async {
        guard let data = raw.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            lastTextDiagnostic += ", json=invalid"
            return
        }
        jsonMessageCount += 1
        let command = json["cmd"] as? String ?? "-"
        let infoCount = (json["info"] as? [Any])?.count ?? -1
        let hasDMV2 = (json["dm_v2"] as? String)?.isEmpty == false
        lastJSONSummary = "cmd=\(command), info=\(infoCount), dm_v2=\(hasDMV2)"

        guard let comment = Self.parseComment(
            json: json,
            emoticons: connection.emoticons,
            receivedAt: Date()
        ) else {
            return
        }
        await append(comment)
    }

    private func handleJSONStream(_ text: String) async {
        for object in Self.extractJSONObjects(text) {
            await handleJson(object)
        }
    }

    static func parseComment(
        json: [String: Any],
        emoticons: [String: DanmakuEmoticon] = [:],
        receivedAt: Date = Date()
    ) -> DanmakuComment? {
        let cmd = json["cmd"] as? String ?? ""
        guard cmd.hasPrefix("DANMU_MSG") else {
            guard let payload = json["dm_v2"] as? String, !payload.isEmpty else { return nil }
            return BiliDMV2Parser.parse(base64: payload, receivedAt: receivedAt)
        }
        guard let info = json["info"] as? [Any] else { return nil }

        let user: String = {
            if let arr = info[safe: 2] as? [Any],
               let name = arr[safe: 1] as? String { return name }
            return ""
        }()
        let info0 = info[safe: 0] as? [Any] ?? []
        let mode = Self.mode(from: Self.intValue(info0[safe: 1]))
        let rawColor = UInt32(clamping: Self.intValue(info0[safe: 3]) ?? 0xFF_FF_FF)
        let color = rawColor & 0xFF_FF_FF
        let alphaByte = rawColor > 0xFF_FF_FF ? Int((rawColor >> 24) & 0xFF) : 255
        let opacity = Double(alphaByte) / 255

        if let emoticon = info0[safe: 13] as? [String: Any],
           let rawURL = emoticon["url"] as? String,
           let url = BiliDanmakuResolver.ensureHTTPS(rawURL) {
            return DanmakuComment(
                text: "",
                user: user,
                imageUrl: url,
                imageWidth: Self.scaledImageWidth(Self.intValue(emoticon["width"])),
                receivedAt: receivedAt,
                colorRGB: color,
                opacity: opacity,
                sourceMode: mode
            )
        }

        if let extraObject = info0[safe: 15] as? [String: Any],
           let extraString = extraObject["extra"] as? String,
           let extraData = extraString.data(using: .utf8),
           let extra = try? JSONSerialization.jsonObject(with: extraData) as? [String: Any] {
            if let emots = extra["emots"] as? [String: Any],
               let first = emots.values.first as? [String: Any],
               let rawURL = first["url"] as? String,
               let url = BiliDanmakuResolver.ensureHTTPS(rawURL) {
                return DanmakuComment(
                    text: "",
                    user: user,
                    imageUrl: url,
                    imageWidth: Self.scaledImageWidth(Self.intValue(first["width"])),
                    receivedAt: receivedAt,
                    colorRGB: color,
                    opacity: opacity,
                    sourceMode: mode
                )
            }
            if let unique = extra["emoticon_unique"] as? String,
               let emoticon = emoticons[unique] {
                return DanmakuComment(
                    text: "",
                    user: user,
                    imageUrl: emoticon.url,
                    imageWidth: Self.scaledImageWidth(emoticon.width),
                    receivedAt: receivedAt,
                    colorRGB: color,
                    opacity: opacity,
                    sourceMode: mode
                )
            }
            if let content = extra["content"] as? String, !content.isEmpty {
                return DanmakuComment(
                    text: content,
                    user: user,
                    receivedAt: receivedAt,
                    colorRGB: color,
                    opacity: opacity,
                    sourceMode: mode
                )
            }
        }

        guard let text = info[safe: 1] as? String, !text.isEmpty else { return nil }
        return DanmakuComment(
            text: text,
            user: user,
            receivedAt: receivedAt,
            colorRGB: color,
            opacity: opacity,
            sourceMode: mode
        )
    }

    static func encodeDouyuPacket(_ message: String) -> Data {
        let body = Data(message.utf8)
        let fullLength = UInt32(body.count + 9)
        var data = Data(capacity: Int(fullLength + 4))
        data.appendUInt32LE(fullLength)
        data.appendUInt32LE(fullLength)
        data.appendUInt16LE(689)
        data.append(0)
        data.append(0)
        data.append(body)
        data.append(0)
        return data
    }

    static func decodeDouyuPackets(_ data: Data) -> [String] {
        var result: [String] = []
        var offset = 0
        while offset + 12 <= data.count {
            let fullLength = Int(data.u32LE(at: offset))
            let totalLength = fullLength + 4
            guard fullLength >= 9, offset + totalLength <= data.count else { break }
            let bodyLength = fullLength - 9
            let start = offset + 12
            result.append(String(decoding: data[start..<(start + bodyLength)], as: UTF8.self))
            offset += totalLength
        }
        return result
    }

    static func parseDouyuComment(
        _ text: String,
        receivedAt: Date = Date()
    ) -> DanmakuComment? {
        guard text.hasPrefix("type@=chatmsg") else { return nil }
        var values: [String: String] = [:]
        for part in text.split(separator: "/", omittingEmptySubsequences: true) {
            guard let range = part.range(of: "@=") else { continue }
            let key = String(part[..<range.lowerBound])
            let rawValue = String(part[range.upperBound...])
            values[key] = rawValue
                .replacingOccurrences(of: "@S", with: "/")
                .replacingOccurrences(of: "@A", with: "@")
        }
        guard let content = values["txt"], !content.isEmpty else { return nil }
        return DanmakuComment(
            text: content,
            user: values["nn"] ?? "",
            receivedAt: receivedAt
        )
    }

    static func encodeHuyaJoin(yyuid: Int64, uid: Int64) -> Data {
        var inner = HuyaJCE.Encoder()
        inner.writeInt64(tag: 0, value: yyuid)
        inner.writeBool(tag: 1, value: true)
        inner.writeString(tag: 2, value: "")
        inner.writeString(tag: 3, value: "")
        inner.writeInt64(tag: 4, value: uid)
        inner.writeInt64(tag: 5, value: uid)
        inner.writeInt32(tag: 6, value: 0)
        inner.writeInt32(tag: 7, value: 0)

        var outer = HuyaJCE.Encoder()
        outer.writeInt32(tag: 0, value: 1)
        outer.writeBytes(tag: 1, value: inner.data)
        return outer.data
    }

    private func append(_ comment: DanmakuComment) async {
        comments.append(comment)
        if comments.count > maxComments {
            comments.removeFirst(comments.count - maxComments)
        }
    }

    // MARK: - 二进制编码/解码

    private func encodePacket(body: Data, operation: UInt32, protover: UInt16 = 1) -> Data {
        let headerLen: UInt16 = 16
        let packetLen = UInt32(headerLen) + UInt32(body.count)
        let seq = sequence; sequence = sequence &+ 1

        var data = Data(capacity: Int(packetLen))
        data.appendUInt32BE(packetLen)
        data.appendUInt16BE(headerLen)
        data.appendUInt16BE(protover)
        data.appendUInt32BE(operation)
        data.appendUInt32BE(seq)
        data.append(body)
        return data
    }

    private struct ParsePacket {
        let protover: UInt16
        let operation: UInt32
        let body: Data
    }

    private func parsePackets(data: Data) -> [ParsePacket] {
        var packets: [ParsePacket] = []
        var offset = 0
        while offset + 16 <= data.count {
            let packetLen = Int(data.u32BE(at: offset))
            guard packetLen >= 16, packetLen <= data.count - offset else { break }
            let headerLen = Int(data.u16BE(at: offset + 4))
            let protover = data.u16BE(at: offset + 6)
            let operation = data.u32BE(at: offset + 8)
            let bodyStart = offset + headerLen
            let bodyEnd = offset + packetLen
            guard headerLen >= 16, bodyEnd <= data.count, bodyStart <= bodyEnd else { break }
            packets.append(ParsePacket(protover: protover, operation: operation, body: data.subdata(in: bodyStart..<bodyEnd)))
            offset += packetLen
        }
        return packets
    }

    private static func hasPacketHeader(_ data: Data) -> Bool {
        guard data.count >= 16 else { return false }
        let packetLength = Int(data.u32BE(at: 0))
        let headerLength = Int(data.u16BE(at: 4))
        return packetLength >= 16 &&
            packetLength <= data.count &&
            headerLength >= 16 &&
            headerLength <= packetLength
    }

    /// 兼容部分节点解压后直接返回连续 JSON，而不是再次套 Bilibili 二进制包头。
    static func extractJSONObjects(_ text: String) -> [String] {
        var results: [String] = []
        var start: String.Index?
        var depth = 0
        var inString = false
        var escaped = false

        for index in text.indices {
            let character = text[index]
            if inString {
                if escaped {
                    escaped = false
                } else if character == "\\" {
                    escaped = true
                } else if character == "\"" {
                    inString = false
                }
                continue
            }
            if character == "\"" {
                inString = true
            } else if character == "{" {
                if depth == 0 { start = index }
                depth += 1
            } else if character == "}", depth > 0 {
                depth -= 1
                if depth == 0, let objectStart = start {
                    results.append(String(text[objectStart...index]))
                    start = nil
                }
            }
        }
        return results
    }

    /// Zlib 解压（使用 Compression framework），逐步扩容，避免高弹幕量帧截断。
    static func inflateZlib(_ data: Data) -> Data? {
        inflate(data, algorithm: COMPRESSION_ZLIB)
    }

    /// Bilibili 的 proto=2 在不同节点可能是 zlib wrapper 或 raw deflate。
    /// 对齐 chaos-core：先尝试完整 zlib，再剥离两字节 wrapper 按 deflate 解。
    static func inflateBiliZlib(_ data: Data) -> Data? {
        let candidates: [Data?] = [
            inflateZlib(data),
            data.count > 2 ? inflateZlib(Data(data.dropFirst(2))) : nil,
        ]
        for candidate in candidates.compactMap({ $0 }) {
            if looksLikeDanmakuPayload(candidate) {
                return candidate
            }
            // 少数节点会在 zlib 外层中继续包一层 Brotli。
            if let nested = inflateBrotli(candidate), looksLikeDanmakuPayload(nested) {
                return nested
            }
        }
        return nil
    }

    static func inflateBrotli(_ data: Data) -> Data? {
        inflate(data, algorithm: COMPRESSION_BROTLI)
    }

    private static func inflate(
        _ data: Data,
        algorithm: compression_algorithm
    ) -> Data? {
        guard !data.isEmpty else { return Data() }
        var destinationSize = max(data.count * 4, 64 * 1024)
        let maximumSize = 16 * 1024 * 1024
        while destinationSize <= maximumSize {
            let decoded: Data? = data.withUnsafeBytes { source in
                guard let sourceAddress = source.baseAddress else { return nil }
                var destination = Data(count: destinationSize)
                let written = destination.withUnsafeMutableBytes { buffer in
                    guard let destinationAddress = buffer.baseAddress else { return 0 }
                    return compression_decode_buffer(
                        destinationAddress.assumingMemoryBound(to: UInt8.self),
                        destinationSize,
                        sourceAddress.assumingMemoryBound(to: UInt8.self),
                        data.count,
                        nil,
                        algorithm
                    )
                }
                // `compression_decode_buffer` 在目标缓冲不足时会返回填满后的长度；
                // 这种情况必须继续扩容，不能把截断数据当作完整帧。
                guard written > 0, written < destinationSize else { return nil }
                return Data(destination.prefix(written))
            }
            if let decoded {
                return decoded
            }
            destinationSize *= 2
        }
        return nil
    }

    private static func looksLikeDanmakuPayload(_ data: Data) -> Bool {
        if hasPacketHeader(data) {
            return true
        }
        guard let text = String(data: data, encoding: .utf8) else { return false }
        return text.drop(while: { $0.isWhitespace || $0 == "\0" }).first == "{"
    }

    private static func isSuccessfulAuthResponse(_ data: Data) -> Bool {
        guard !data.isEmpty,
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return true
        }
        return intValue(object["code"]) == 0
    }

    private static func mode(from value: Int?) -> DanmakuMode {
        switch value {
        case 4: return .bottom
        case 5: return .top
        default: return .scroll
        }
    }

    private static func intValue(_ value: Any?) -> Int? {
        if let value = value as? Int { return value }
        if let value = value as? Int64 { return Int(value) }
        if let value = value as? NSNumber { return value.intValue }
        if let value = value as? String { return Int(value) }
        return nil
    }

    private static func scaledImageWidth(_ width: Int?) -> Int? {
        guard let width, width > 0 else { return nil }
        return max(24, min(width, 200) / 2)
    }

    private static func makeFallbackBuvid() -> String {
        UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased() + "12345infoc"
    }
}

// MARK: - Data 扩展（大端序读写）

private extension Data {
    func u32BE(at offset: Int) -> UInt32 {
        (UInt32(self[offset]) << 24) | (UInt32(self[offset+1]) << 16) | (UInt32(self[offset+2]) << 8) | UInt32(self[offset+3])
    }

    func u16BE(at offset: Int) -> UInt16 {
        (UInt16(self[offset]) << 8) | UInt16(self[offset+1])
    }

    mutating func appendUInt32BE(_ value: UInt32) {
        append(UInt8((value >> 24) & 0xFF))
        append(UInt8((value >> 16) & 0xFF))
        append(UInt8((value >> 8) & 0xFF))
        append(UInt8(value & 0xFF))
    }

    mutating func appendUInt16BE(_ value: UInt16) {
        append(UInt8((value >> 8) & 0xFF))
        append(UInt8(value & 0xFF))
    }

    func u32LE(at offset: Int) -> UInt32 {
        UInt32(self[offset]) |
            (UInt32(self[offset + 1]) << 8) |
            (UInt32(self[offset + 2]) << 16) |
            (UInt32(self[offset + 3]) << 24)
    }

    mutating func appendUInt32LE(_ value: UInt32) {
        append(UInt8(value & 0xFF))
        append(UInt8((value >> 8) & 0xFF))
        append(UInt8((value >> 16) & 0xFF))
        append(UInt8((value >> 24) & 0xFF))
    }

    mutating func appendUInt16LE(_ value: UInt16) {
        append(UInt8(value & 0xFF))
        append(UInt8((value >> 8) & 0xFF))
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
