import SwiftUI

/// 全局状态：当前 Toast 消息。
@MainActor
public final class AppState: ObservableObject {
    @Published public var toastMessage: String?
    @Published public var toastTask: Task<Void, Never>?

    public init() {}

    /// 展示一条 Toast，2.2s 后自动消失（对齐原型 setTimeout 2200ms）。
    public func showToast(_ message: String) {
        toastTask?.cancel()
        toastMessage = message
        toastTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 2_200_000_000)
            guard !Task.isCancelled else { return }
            await MainActor.run { self?.toastMessage = nil }
        }
    }
}

/// 侧边栏导航项。
public enum NavDestination: String, Hashable, CaseIterable, Identifiable {
    case home, history, settings

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .home: return "首页"
        case .history: return "历史"
        case .settings: return "设置"
        }
    }

    public var systemImage: String {
        switch self {
        case .home: return "house"
        case .history: return "clock"
        case .settings: return "gearshape"
        }
    }
}

/// 主题偏好（持久化到 UserDefaults）。
public enum AppAppearance: String, CaseIterable {
    case system, light, dark

    public var colorScheme: ColorScheme? {
        switch self {
        case .system: return nil
        case .light: return .light
        case .dark: return .dark
        }
    }

    public var label: String {
        switch self {
        case .system: return "跟随系统"
        case .light: return "浅色"
        case .dark: return "深色"
        }
    }
}
