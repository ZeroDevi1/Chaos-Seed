import SwiftUI
import SwiftData

@main
struct ChaosSeedApp: App {
    @StateObject private var appState = AppState()
    @AppStorage("appearance") private var appearanceRaw: String = AppAppearance.system.rawValue
    /// 是否使用 Mock 数据（调试用，默认 false = 真实接口解析）。
    @AppStorage("useMockLiveKit") private var useMockLiveKit: Bool = false
    @State private var selection: NavDestination = .home
    @State private var openedRoom: LiveRoomCard?
    @State private var isWindowFullScreen = false

    // 真实网络层：直接调用 BiliLive/Douyu/Huya 接口（移植自 chaos-core）。
    private let realLiveKit = RealLiveKit()
    private let mockLiveKit = MockLiveKit(simulateLatency: true)
    private let historyContainer: ModelContainer

    /// HomeViewModel 提升到 App 层持久化，避免进出详情时 HomeView 被销毁导致
    /// 平台/分类/分页状态丢失（任务1：记忆状态）。
    @StateObject private var homeVM: HomeViewModel

    init() {
        do {
            historyContainer = try ModelContainer(for: PlaybackHistoryEntry.self)
        } catch {
            Log.app.error("初始化历史记录数据库失败，已回退到内存存储", error: error)
            let fallback = ModelConfiguration(isStoredInMemoryOnly: true)
            historyContainer = try! ModelContainer(
                for: PlaybackHistoryEntry.self,
                configurations: fallback
            )
        }

        // useMockLiveKit 在 AppStorage 初始化前读取默认值（首次启动为 false）。
        let useMock = UserDefaults.standard.bool(forKey: "useMockLiveKit")
        let kit: LiveKit = useMock
            ? MockLiveKit(simulateLatency: true)
            : RealLiveKit()
        _homeVM = StateObject(wrappedValue: HomeViewModel(liveKit: kit))
    }

    var body: some Scene {
        WindowGroup("") {
            ZStack {
                RootView(
                    liveKit: liveKit,
                    homeVM: homeVM,
                    selection: $selection,
                    appearance: Binding(
                        get: { AppAppearance(rawValue: appearanceRaw) ?? .system },
                        set: { appearanceRaw = $0.rawValue }
                    ),
                    onOpenRoom: { openedRoom = $0 }
                )
                if let room = openedRoom {
                    DetailView(liveKit: liveKit, room: room, onBack: { openedRoom = nil })
                        .transition(.opacity)
                }

                if let toast = appState.toastMessage {
                    ToastView(message: toast)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                        .frame(maxHeight: .infinity, alignment: .bottom)
                        .padding(.bottom, 24)
                }
            }
            .animation(.easeInOut(duration: 0.18), value: openedRoom)
            .animation(.easeOut(duration: 0.2), value: appState.toastMessage)
            .environmentObject(appState)
            .preferredColorScheme(AppAppearance(rawValue: appearanceRaw)?.colorScheme)
            .frame(minWidth: 900, minHeight: 600)
            .background(WindowChromeConfigurator())
            .toolbarVisibility(
                isWindowFullScreen && openedRoom != nil ? .hidden : .automatic,
                for: .windowToolbar
            )
            .windowToolbarFullScreenVisibility(.onHover)
            .onReceive(
                NotificationCenter.default.publisher(
                    for: NSWindow.didEnterFullScreenNotification
                )
            ) { _ in
                isWindowFullScreen = true
            }
            .onReceive(
                NotificationCenter.default.publisher(
                    for: NSWindow.didExitFullScreenNotification
                )
            ) { _ in
                isWindowFullScreen = false
            }
        }
        .windowStyle(.titleBar)
        .defaultSize(width: 1020, height: 680)
        .modelContainer(historyContainer)
    }

    /// 根据 `useMockLiveKit` 切换真实/Mock 网络层。
    private var liveKit: LiveKit {
        useMockLiveKit ? mockLiveKit : realLiveKit
    }
}

/// 统一窗口左上角 titlebar 的视觉。
///
/// 系统的红黄绿、sidebar toggle 和标题都属于 AppKit titlebar。侧边栏展开/收起时，
/// titlebar 下方背景会从 sidebar 材质切到内容区白底，容易出现两套观感。这里不重绘
/// 系统控件，只把 titlebar 调整为透明 compact toolbar，让系统按钮像参考图一样悬在
/// 同一层玻璃区域上。
private struct WindowChromeConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        DispatchQueue.main.async {
            configure(window: view.window)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            configure(window: nsView.window)
        }
    }

    private func configure(window: NSWindow?) {
        guard let window else { return }
        window.title = ""
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.toolbarStyle = .unifiedCompact
        window.styleMask.insert(.fullSizeContentView)
    }
}
