import Foundation

/// BiliLive `dm_v2` 的最小 protobuf 解码器。
///
/// 字段与 `chaos-core/src/danmaku/proto/bili_live_dm_v2.rs` 一致，仅解析播放器需要的
/// text、biz_scene、dm_type 与首个表情 URL/宽度，未知字段按 protobuf wire type 跳过。
enum BiliDMV2Parser {
    static func parse(base64: String, receivedAt: Date) -> DanmakuComment? {
        guard let data = Data(base64Encoded: base64) else { return nil }
        var reader = ProtoReader(data)
        var text = ""
        var bizScene = 0
        var dmType = 0
        var emoticon: (url: String, width: Int)?

        while let tag = reader.readTag() {
            switch (tag.field, tag.wireType) {
            case (6, 2):
                text = reader.readString() ?? text
            case (11, 0):
                bizScene = Int(reader.readVarint() ?? 0)
            case (13, 0):
                dmType = Int(reader.readVarint() ?? 0)
            case (14, 2):
                if let payload = reader.readLengthDelimited(),
                   let value = parseEmoticonEntry(payload) {
                    emoticon = value
                }
            default:
                guard reader.skip(wireType: tag.wireType) else { return nil }
            }
        }

        // 与 main 分支一致：生存场景不是普通播放器弹幕。
        guard bizScene != 2 else { return nil }
        if dmType == 1, let emoticon,
           let url = BiliDanmakuResolver.ensureHTTPS(emoticon.url) {
            return DanmakuComment(
                text: "",
                imageUrl: url,
                imageWidth: scaledWidth(emoticon.width),
                receivedAt: receivedAt
            )
        }
        guard !text.isEmpty else { return nil }
        return DanmakuComment(text: text, receivedAt: receivedAt)
    }

    private static func parseEmoticonEntry(_ data: Data) -> (url: String, width: Int)? {
        var entry = ProtoReader(data)
        while let tag = entry.readTag() {
            if tag.field == 2, tag.wireType == 2,
               let payload = entry.readLengthDelimited() {
                return parseEmoticon(payload)
            }
            guard entry.skip(wireType: tag.wireType) else { return nil }
        }
        return nil
    }

    private static func parseEmoticon(_ data: Data) -> (url: String, width: Int)? {
        var reader = ProtoReader(data)
        var url = ""
        var width = 0
        while let tag = reader.readTag() {
            switch (tag.field, tag.wireType) {
            case (2, 2):
                url = reader.readString() ?? url
            case (7, 0):
                width = Int(reader.readVarint() ?? 0)
            default:
                guard reader.skip(wireType: tag.wireType) else { return nil }
            }
        }
        return url.isEmpty ? nil : (url, width)
    }

    private static func scaledWidth(_ width: Int) -> Int? {
        guard width > 0 else { return nil }
        return max(24, min(width, 200) / 2)
    }
}

private struct ProtoReader {
    private let bytes: [UInt8]
    private var index = 0

    init(_ data: Data) {
        self.bytes = Array(data)
    }

    mutating func readTag() -> (field: Int, wireType: Int)? {
        guard index < bytes.count, let value = readVarint() else { return nil }
        let field = Int(value >> 3)
        let wireType = Int(value & 0x07)
        return field > 0 ? (field, wireType) : nil
    }

    mutating func readVarint() -> UInt64? {
        var result: UInt64 = 0
        var shift: UInt64 = 0
        while index < bytes.count, shift < 64 {
            let byte = bytes[index]
            index += 1
            result |= UInt64(byte & 0x7F) << shift
            if byte & 0x80 == 0 {
                return result
            }
            shift += 7
        }
        return nil
    }

    mutating func readString() -> String? {
        guard let data = readLengthDelimited() else { return nil }
        return String(data: data, encoding: .utf8)
    }

    mutating func readLengthDelimited() -> Data? {
        guard let rawLength = readVarint(), rawLength <= UInt64(Int.max) else { return nil }
        let length = Int(rawLength)
        guard length >= 0, index + length <= bytes.count else { return nil }
        defer { index += length }
        return Data(bytes[index..<(index + length)])
    }

    mutating func skip(wireType: Int) -> Bool {
        switch wireType {
        case 0:
            return readVarint() != nil
        case 1:
            return advance(8)
        case 2:
            guard let length = readVarint(), length <= UInt64(Int.max) else { return false }
            return advance(Int(length))
        case 5:
            return advance(4)
        default:
            return false
        }
    }

    private mutating func advance(_ count: Int) -> Bool {
        guard count >= 0, index + count <= bytes.count else { return false }
        index += count
        return true
    }
}
