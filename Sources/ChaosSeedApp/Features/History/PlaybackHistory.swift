import Foundation
import SwiftData

public enum PlaybackMethod: String, Codable, CaseIterable, Sendable {
    case builtin
    case iina

    public var label: String {
        switch self {
        case .builtin: return "内置播放器"
        case .iina: return "IINA"
        }
    }
}

/// 本地观看历史。只保存重新进入房间需要的信息，不保存会过期的直播流 URL。
@Model
public final class PlaybackHistoryEntry {
    @Attribute(.unique) public var roomKey: String
    public var siteRawValue: String
    public var roomId: String
    public var input: String
    public var title: String
    public var userName: String?
    public var cover: String?
    public var playedAt: Date
    public var playbackMethodRawValue: String
    public var qualityLabel: String?

    public init(
        roomKey: String,
        siteRawValue: String,
        roomId: String,
        input: String,
        title: String,
        userName: String?,
        cover: String?,
        playedAt: Date,
        playbackMethodRawValue: String,
        qualityLabel: String?
    ) {
        self.roomKey = roomKey
        self.siteRawValue = siteRawValue
        self.roomId = roomId
        self.input = input
        self.title = title
        self.userName = userName
        self.cover = cover
        self.playedAt = playedAt
        self.playbackMethodRawValue = playbackMethodRawValue
        self.qualityLabel = qualityLabel
    }

    public var site: Site {
        Site(rawValue: siteRawValue) ?? .biliLive
    }

    public var playbackMethod: PlaybackMethod {
        PlaybackMethod(rawValue: playbackMethodRawValue) ?? .builtin
    }

    public var room: LiveRoomCard {
        LiveRoomCard(
            site: site,
            roomId: roomId,
            input: input,
            title: title,
            cover: cover,
            userName: userName
        )
    }
}

@MainActor
enum PlaybackHistoryStore {
    static let maximumEntryCount = 200

    static func roomKey(site: Site, roomId: String) -> String {
        "\(site.rawKey):\(roomId)"
    }

    static func record(
        room: LiveRoomCard,
        method: PlaybackMethod,
        qualityLabel: String?,
        playedAt: Date = .now,
        in context: ModelContext
    ) throws {
        let key = roomKey(site: room.site, roomId: room.roomId)
        var descriptor = FetchDescriptor<PlaybackHistoryEntry>(
            predicate: #Predicate { $0.roomKey == key }
        )
        descriptor.fetchLimit = 1

        if let entry = try context.fetch(descriptor).first {
            entry.input = room.input
            entry.title = room.title
            entry.userName = room.userName
            entry.cover = room.cover
            entry.playedAt = playedAt
            entry.playbackMethodRawValue = method.rawValue
            entry.qualityLabel = qualityLabel
        } else {
            context.insert(
                PlaybackHistoryEntry(
                    roomKey: key,
                    siteRawValue: room.site.rawValue,
                    roomId: room.roomId,
                    input: room.input,
                    title: room.title,
                    userName: room.userName,
                    cover: room.cover,
                    playedAt: playedAt,
                    playbackMethodRawValue: method.rawValue,
                    qualityLabel: qualityLabel
                )
            )
        }

        try trim(in: context)
        try context.save()
    }

    static func delete(_ entry: PlaybackHistoryEntry, in context: ModelContext) throws {
        context.delete(entry)
        try context.save()
    }

    static func deleteAll(in context: ModelContext) throws {
        try context.delete(model: PlaybackHistoryEntry.self)
        try context.save()
    }

    static func fetchAll(in context: ModelContext) throws -> [PlaybackHistoryEntry] {
        let descriptor = FetchDescriptor<PlaybackHistoryEntry>(
            sortBy: [SortDescriptor(\.playedAt, order: .reverse)]
        )
        return try context.fetch(descriptor)
    }

    private static func trim(in context: ModelContext) throws {
        let entries = try fetchAll(in: context)
        guard entries.count > maximumEntryCount else { return }
        for entry in entries.dropFirst(maximumEntryCount) {
            context.delete(entry)
        }
    }
}
