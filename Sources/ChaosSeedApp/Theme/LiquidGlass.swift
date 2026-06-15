import SwiftUI

/// Liquid Glass 材质封装。
///
/// 应用最低支持 macOS 26，因此这里直接使用 SwiftUI 新 API，不再维护旧系统回退分支。
extension View {
    /// 为卡片或浮层应用 Liquid Glass 背景。
    func liquidGlassBackground<S: Shape>(in shape: S = .rect) -> some View {
        glassEffect(in: shape)
    }

    /// 为工具栏应用连续的 Liquid Glass 背景。
    func liquidGlassBar() -> some View {
        glassEffect(in: .rect)
    }
}
