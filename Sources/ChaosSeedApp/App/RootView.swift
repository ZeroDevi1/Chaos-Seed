import SwiftUI

/// 应用根视图：侧边栏导航 + 内容区，对齐原型 `app.jsx` 的 main-layout。
public struct RootView: View {
    @EnvironmentObject private var appState: AppState
    @Binding private var selection: NavDestination
    @Binding private var appearance: AppAppearance

    private let liveKit: LiveKit
    private let homeVM: HomeViewModel
    private let onOpenRoom: (LiveRoomCard) -> Void

    public init(
        liveKit: LiveKit,
        homeVM: HomeViewModel,
        selection: Binding<NavDestination>,
        appearance: Binding<AppAppearance>,
        onOpenRoom: @escaping (LiveRoomCard) -> Void
    ) {
        self.liveKit = liveKit
        self.homeVM = homeVM
        self._selection = selection
        self._appearance = appearance
        self.onOpenRoom = onOpenRoom
    }

    public var body: some View {
        NavigationSplitView {
            // 侧边栏：导航项列表（macOS 26 使用 Liquid Glass 材质）。
            List(selection: $selection) {
                Section("导航") {
                    ForEach(NavDestination.allCases) { item in
                        Label(item.label, systemImage: item.systemImage)
                            .tag(item)
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 200)
            .listStyle(.sidebar)
            .liquidGlassBackground(in: .rect)
        } detail: {
            switch selection {
            case .home:
                HomeView(liveKit: liveKit, homeVM: homeVM, onOpenRoom: onOpenRoom)
            case .history:
                HistoryView(onOpenRoom: onOpenRoom)
            case .settings:
                SettingsView(appearance: $appearance)
            }
        }
        .frame(minWidth: 900, minHeight: 600)
    }
}

/// 底部 Toast 提示（对齐原型 `.toast`，macOS 26 使用 Liquid Glass 材质）。
struct ToastView: View {
    let message: String

    var body: some View {
        Text(message)
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(.white)
            .padding(.horizontal, 18)
            .padding(.vertical, 10)
            .liquidGlassBackground(in: .capsule)
            .shadow(color: .black.opacity(0.25), radius: 12, y: 4)
    }
}
