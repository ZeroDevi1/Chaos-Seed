import XCTest
import SwiftData
@testable import ChaosSeedApp

@MainActor
final class PlaybackHistoryTests: XCTestCase {
    private func makeContainer() throws -> ModelContainer {
        let configuration = ModelConfiguration(isStoredInMemoryOnly: true)
        return try ModelContainer(
            for: PlaybackHistoryEntry.self,
            configurations: configuration
        )
    }

    private func room(id: Int, site: Site = .biliLive) -> LiveRoomCard {
        LiveRoomCard(
            site: site,
            roomId: "\(id)",
            input: site.makeInput(roomId: "\(id)"),
            title: "房间 \(id)",
            cover: "https://example.com/\(id).jpg",
            userName: "主播 \(id)"
        )
    }

    func testRecordCreatesAndUpdatesOneEntryPerRoom() throws {
        let container = try makeContainer()
        let context = container.mainContext
        let original = room(id: 1)

        try PlaybackHistoryStore.record(
            room: original,
            method: .builtin,
            qualityLabel: "蓝光",
            playedAt: Date(timeIntervalSince1970: 100),
            in: context
        )

        var updated = original
        updated.title = "更新后的标题"
        try PlaybackHistoryStore.record(
            room: updated,
            method: .iina,
            qualityLabel: "原画",
            playedAt: Date(timeIntervalSince1970: 200),
            in: context
        )

        let entries = try PlaybackHistoryStore.fetchAll(in: context)
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0].title, "更新后的标题")
        XCTAssertEqual(entries[0].playbackMethod, .iina)
        XCTAssertEqual(entries[0].qualityLabel, "原画")
        XCTAssertEqual(entries[0].playedAt, Date(timeIntervalSince1970: 200))
    }

    func testRecordSortsNewestFirstAndTrimsToTwoHundredEntries() throws {
        let container = try makeContainer()
        let context = container.mainContext

        for index in 0...PlaybackHistoryStore.maximumEntryCount {
            try PlaybackHistoryStore.record(
                room: room(id: index),
                method: .builtin,
                qualityLabel: nil,
                playedAt: Date(timeIntervalSince1970: TimeInterval(index)),
                in: context
            )
        }

        let entries = try PlaybackHistoryStore.fetchAll(in: context)
        XCTAssertEqual(entries.count, PlaybackHistoryStore.maximumEntryCount)
        XCTAssertEqual(entries.first?.roomId, "\(PlaybackHistoryStore.maximumEntryCount)")
        XCTAssertEqual(entries.last?.roomId, "1")
    }

    func testDeleteAndDeleteAll() throws {
        let container = try makeContainer()
        let context = container.mainContext

        try PlaybackHistoryStore.record(
            room: room(id: 1),
            method: .builtin,
            qualityLabel: nil,
            in: context
        )
        try PlaybackHistoryStore.record(
            room: room(id: 2),
            method: .iina,
            qualityLabel: nil,
            in: context
        )

        let first = try XCTUnwrap(PlaybackHistoryStore.fetchAll(in: context).first)
        try PlaybackHistoryStore.delete(first, in: context)
        XCTAssertEqual(try PlaybackHistoryStore.fetchAll(in: context).count, 1)

        try PlaybackHistoryStore.deleteAll(in: context)
        XCTAssertTrue(try PlaybackHistoryStore.fetchAll(in: context).isEmpty)
    }
}
