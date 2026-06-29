import Foundation

enum HuyaJCE {
    private static let byte: UInt8 = 0
    private static let short: UInt8 = 1
    private static let int: UInt8 = 2
    private static let long: UInt8 = 3
    private static let string1: UInt8 = 6
    private static let string4: UInt8 = 7
    private static let list: UInt8 = 9
    private static let structBegin: UInt8 = 10
    private static let structEnd: UInt8 = 11
    private static let zeroTag: UInt8 = 12
    private static let simpleList: UInt8 = 13

    struct Encoder {
        private(set) var data = Data()

        mutating func writeBool(tag: UInt8, value: Bool) {
            guard value else {
                writeHead(tag: tag, type: zeroTag)
                return
            }
            writeHead(tag: tag, type: byte)
            data.append(1)
        }

        mutating func writeInt32(tag: UInt8, value: Int32) {
            guard value != 0 else {
                writeHead(tag: tag, type: zeroTag)
                return
            }
            writeHead(tag: tag, type: int)
            appendBigEndian(value)
        }

        mutating func writeInt64(tag: UInt8, value: Int64) {
            guard value != 0 else {
                writeHead(tag: tag, type: zeroTag)
                return
            }
            writeHead(tag: tag, type: long)
            appendBigEndian(value)
        }

        mutating func writeString(tag: UInt8, value: String) {
            let bytes = Data(value.utf8)
            if bytes.count < 255 {
                writeHead(tag: tag, type: string1)
                data.append(UInt8(bytes.count))
            } else {
                writeHead(tag: tag, type: string4)
                appendBigEndian(Int32(clamping: bytes.count))
            }
            data.append(bytes)
        }

        mutating func writeBytes(tag: UInt8, value: Data) {
            writeHead(tag: tag, type: simpleList)
            writeHead(tag: 0, type: byte)
            writeHead(tag: 0, type: int)
            appendBigEndian(Int32(clamping: value.count))
            data.append(value)
        }

        private mutating func writeHead(tag: UInt8, type: UInt8) {
            if tag < 15 {
                data.append((tag << 4) | (type & 0x0F))
            } else {
                data.append(0xF0 | (type & 0x0F))
                data.append(tag)
            }
        }

        private mutating func appendBigEndian<T: FixedWidthInteger>(_ value: T) {
            var value = value.bigEndian
            withUnsafeBytes(of: &value) { data.append(contentsOf: $0) }
        }
    }

    static func int32(_ data: Data, tag: UInt32) throws -> Int32? {
        var reader = Reader(data)
        guard try reader.skip(to: tag) else { return nil }
        let head = try reader.readHead()
        return Int32(clamping: try reader.readInteger(type: head.type))
    }

    static func int64(_ data: Data, tag: UInt32) throws -> Int64? {
        var reader = Reader(data)
        guard try reader.skip(to: tag) else { return nil }
        let head = try reader.readHead()
        return try reader.readInteger(type: head.type)
    }

    static func string(_ data: Data, tag: UInt32) throws -> String? {
        var reader = Reader(data)
        guard try reader.skip(to: tag) else { return nil }
        let head = try reader.readHead()
        switch head.type {
        case string1:
            return String(decoding: try reader.readData(count: Int(try reader.readUInt8())), as: UTF8.self)
        case string4:
            return String(decoding: try reader.readData(count: Int(try reader.readInt32BE())), as: UTF8.self)
        case zeroTag:
            return ""
        default:
            throw LiveKitError.parse("huya jce string type mismatch")
        }
    }

    static func bytes(_ data: Data, tag: UInt32) throws -> Data? {
        var reader = Reader(data)
        guard try reader.skip(to: tag) else { return nil }
        let head = try reader.readHead()
        switch head.type {
        case simpleList:
            let marker = try reader.readHead()
            guard marker.type == byte else {
                throw LiveKitError.parse("huya jce simple list marker mismatch")
            }
            let size = try reader.readInteger(type: try reader.readHead().type)
            return try reader.readData(count: Int(size))
        case zeroTag:
            return Data()
        default:
            throw LiveKitError.parse("huya jce bytes type mismatch")
        }
    }

    static func structBytes(_ data: Data, tag: UInt32) throws -> Data? {
        var reader = Reader(data)
        guard try reader.skip(to: tag) else { return nil }
        guard try reader.readHead().type == structBegin else {
            throw LiveKitError.parse("huya jce struct type mismatch")
        }
        let start = reader.position
        while true {
            let head = try reader.peekHead()
            if head.type == structEnd {
                return data.subdata(in: start..<reader.position)
            }
            _ = try reader.readHead()
            try reader.skipField(type: head.type)
        }
    }

    private struct Head {
        let type: UInt8
        let tag: UInt32
    }

    private struct Reader {
        let data: Data
        var position = 0

        init(_ data: Data) {
            self.data = data
        }

        mutating func readUInt8() throws -> UInt8 {
            guard position < data.count else {
                throw LiveKitError.parse("huya jce unexpected eof")
            }
            defer { position += 1 }
            return data[position]
        }

        mutating func readInt16BE() throws -> Int16 {
            Int16(bitPattern: UInt16(try readUInt8()) << 8 | UInt16(try readUInt8()))
        }

        mutating func readInt32BE() throws -> Int32 {
            var value: UInt32 = 0
            for _ in 0..<4 { value = (value << 8) | UInt32(try readUInt8()) }
            return Int32(bitPattern: value)
        }

        mutating func readInt64BE() throws -> Int64 {
            var value: UInt64 = 0
            for _ in 0..<8 { value = (value << 8) | UInt64(try readUInt8()) }
            return Int64(bitPattern: value)
        }

        mutating func readData(count: Int) throws -> Data {
            guard count >= 0, position + count <= data.count else {
                throw LiveKitError.parse("huya jce payload out of range")
            }
            defer { position += count }
            return data.subdata(in: position..<(position + count))
        }

        mutating func readHead() throws -> Head {
            let value = try readUInt8()
            let type = value & 0x0F
            let shortTag = value >> 4
            let tag = shortTag == 15 ? UInt32(try readUInt8()) : UInt32(shortTag)
            return Head(type: type, tag: tag)
        }

        func peekHead() throws -> Head {
            var copy = self
            return try copy.readHead()
        }

        mutating func skip(to target: UInt32) throws -> Bool {
            while position < data.count {
                let head = try peekHead()
                if head.type == structEnd || head.tag > target { return false }
                if head.tag == target { return true }
                _ = try readHead()
                try skipField(type: head.type)
            }
            return false
        }

        mutating func readInteger(type: UInt8) throws -> Int64 {
            switch type {
            case zeroTag: return 0
            case byte: return Int64(Int8(bitPattern: try readUInt8()))
            case short: return Int64(try readInt16BE())
            case int: return Int64(try readInt32BE())
            case long: return try readInt64BE()
            default: throw LiveKitError.parse("huya jce integer type mismatch")
            }
        }

        mutating func skipField(type: UInt8) throws {
            switch type {
            case zeroTag, structEnd:
                return
            case byte:
                _ = try readData(count: 1)
            case short:
                _ = try readData(count: 2)
            case int:
                _ = try readData(count: 4)
            case long:
                _ = try readData(count: 8)
            case string1:
                _ = try readData(count: Int(try readUInt8()))
            case string4:
                _ = try readData(count: Int(try readInt32BE()))
            case list:
                let count = try readInteger(type: try readHead().type)
                for _ in 0..<count {
                    let head = try readHead()
                    try skipField(type: head.type)
                }
            case simpleList:
                guard try readHead().type == byte else {
                    throw LiveKitError.parse("huya jce invalid simple list")
                }
                let count = try readInteger(type: try readHead().type)
                _ = try readData(count: Int(count))
            case structBegin:
                while true {
                    let head = try readHead()
                    if head.type == structEnd { break }
                    try skipField(type: head.type)
                }
            default:
                throw LiveKitError.parse("huya jce unsupported type \(type)")
            }
        }
    }
}
