import SwiftUI

@main
struct ChaosSeedApp: App {
    @StateObject private var appState = AppState()
    @AppStorage("appearance") private var appearanceRaw: String = AppAppearance.system.rawValue
    /// 是否使用 Mock 数据（调试用，默认 false = 真实接口解析）。
    @AppStorage("useMockLiveKit") private var useMockLiveKit: Bool = false
    @State private var selection: NavDestination = .home
    @State private var openedRoom: LiveRoomCard?

    // 真实网络层：直接调用 BiliLive/Douyu/Huya 接口（移植自 chaos-core）。
    private let realLiveKit = RealLiveKit()
    private let mockLiveKit = MockLiveKit(simulateLatency: true)

    var body: some Scene {
        WindowGroup("Chaos Seed") {
            Group {
                if let room = openedRoom {
                    DetailView(liveKit: liveKit, room: room, onBack: { openedRoom = nil })
                } else {
                    RootView(
                        liveKit: liveKit,
                        selection: $selection,
                        appearance: Binding(
                            get: { AppAppearance(rawValue: appearanceRaw) ?? .system },
                            set: { appearanceRaw = $0.rawValue }
                        ),
                        onOpenRoom: { openedRoom = $0 }
                    )
                }
            }
            .environmentObject(appState)
            .preferredColorScheme(AppAppearance(rawValue: appearanceRaw)?.colorScheme)
            .frame(minWidth: 900, minHeight: 600)
        }
        .windowStyle(.titleBar)
        .defaultSize(width: 1020, height: 680)
    }

    /// 根据 `useMockLiveKit` 切换真实/Mock 网络层。
    private var liveKit: LiveKit {
        useMockLiveKit ? mockLiveKit : realLiveKit
    }
}
