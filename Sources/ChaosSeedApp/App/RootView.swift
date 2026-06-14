import SwiftUI

/// 应用根视图：侧边栏导航 + 内容区，对齐原型 `app.jsx` 的 main-layout。
public struct RootView: View {
    @EnvironmentObject private var appState: AppState
    @Binding private var selection: NavDestination
    @Binding private var appearance: AppAppearance

    private let liveKit: LiveKit
    private let onOpenRoom: (LiveRoomCard) -> Void

    public init(
        liveKit: LiveKit,
        selection: Binding<NavDestination>,
        appearance: Binding<AppAppearance>,
        onOpenRoom: @escaping (LiveRoomCard) -> Void
    ) {
        self.liveKit = liveKit
        self._selection = selection
        self._appearance = appearance
        self.onOpenRoom = onOpenRoom
    }

    public var body: some View {
        NavigationSplitView {
            // 侧边栏：导航项列表。
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
        } detail: {
            switch selection {
            case .home:
                HomeView(liveKit: liveKit, onOpenRoom: onOpenRoom)
            case .history:
                HistoryView()
            case .settings:
                SettingsView(appearance: $appearance)
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .overlay(alignment: .bottom) {
            if let toast = appState.toastMessage {
                ToastView(message: toast)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .padding(.bottom, 24)
            }
        }
        .animation(.easeOut(duration: 0.2), value: appState.toastMessage)
    }
}

/// 底部 Toast 提示（对齐原型 `.toast`）。
private struct ToastView: View {
    let message: String

    var body: some View {
        Text(message)
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(.white)
            .padding(.horizontal, 18)
            .padding(.vertical, 10)
            .background(
                .ultraThinMaterial,
                in: RoundedRectangle(cornerRadius: 10, style: .continuous)
            )
            .background(Color.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .shadow(color: .black.opacity(0.25), radius: 12, y: 4)
    }
}
