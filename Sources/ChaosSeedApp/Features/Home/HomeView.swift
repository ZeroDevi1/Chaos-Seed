import SwiftUI

/// 首页：平台目录 + 搜索 + 直播间卡片网格。
///
/// 复刻原型 `HomeView.jsx`。
public struct HomeView: View {
    @EnvironmentObject private var appState: AppState
    @ObservedObject private var vm: HomeViewModel
    @State private var showUrlModal = false
    @State private var urlInput = ""
    @State private var urlParsing = false

    private let onOpenRoom: (LiveRoomCard) -> Void

    public init(liveKit: LiveKit, homeVM: HomeViewModel, onOpenRoom: @escaping (LiveRoomCard) -> Void) {
        self._vm = ObservedObject(wrappedValue: homeVM)
        self.onOpenRoom = onOpenRoom
    }

    public var body: some View {
        VStack(spacing: 0) {
            toolbar
            if vm.searchKeyword.isEmpty {
                CategoryBar(
                    categories: vm.categories,
                    selectedCategory: vm.category,
                    selectedSub: vm.subCategory,
                    onSelectCategory: { vm.selectCategory($0) },
                    onSelectSub: { vm.selectSub($0) }
                )
                .padding(.horizontal, 20)
                .padding(.top, 10)
                .padding(.bottom, 6)
                .liquidGlassBar()
            }
            Divider()
            content
        }
        .sheet(isPresented: $showUrlModal) {
            URLParseModal(
                isPresented: $showUrlModal,
                input: $urlInput,
                isParsing: urlParsing,
                onConfirm: parseUrl
            )
        }
        .onAppear {
            // 只在首次（categories 为空）时 bootstrap，避免进出详情重复加载。
            if vm.categories.isEmpty { vm.bootstrap() }
        }
    }

    // MARK: 工具栏

    private var toolbar: some View {
        HStack(spacing: 12) {
            Text("首页")
                .font(.system(size: 16, weight: .bold))
            Picker("平台", selection: Binding(get: { vm.platform }, set: vm.selectPlatform)) {
                ForEach(Site.allCases) { site in
                    Text(site.displayName).tag(site)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 240)
            Spacer(minLength: 0)
            SearchBox(text: $vm.keyword, onSubmit: vm.submitSearch)
            Button("解析 URL") { showUrlModal = true }
            Button {
                vm.refresh()
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .help("刷新")
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
        .liquidGlassBar()
    }

    // MARK: 内容区

    @ViewBuilder
    private var content: some View {
        if vm.loading {
            LoadingSpinner()
        } else if let err = vm.errorMessage {
            EmptyStateView(title: "加载失败", description: err, systemImage: "exclamationmark.triangle")
        } else if vm.rooms.isEmpty {
            EmptyStateView(
                title: vm.searchKeyword.isEmpty ? "暂无直播间" : "未找到匹配结果",
                description: vm.searchKeyword.isEmpty ? "当前分类下没有可展示的直播间。" : "换个关键词试试看。",
                systemImage: "magnifyingglass"
            )
        } else {
            ScrollView {
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: 220), spacing: 16)],
                    spacing: 16
                ) {
                    ForEach(vm.rooms) { room in
                        RoomCard(room: room) { onOpenRoom(room) }
                    }
                }
                .padding(20)
                PaginationBar(
                    page: vm.page,
                    totalPages: vm.totalPages,
                    onPrev: { vm.setPage(vm.page - 1) },
                    onNext: { vm.setPage(vm.page + 1) }
                )
            }
        }
    }

    // MARK: URL 解析

    private func parseUrl() {
        let trimmed = urlInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        urlParsing = true
        Task {
            do {
                let manifest = try await vm.kit.decodeManifest(input: trimmed, options: .default)
                let card = LiveRoomCard(
                    site: manifest.site,
                    roomId: manifest.roomId,
                    input: manifest.rawInput,
                    title: manifest.info.title,
                    cover: manifest.info.cover,
                    userName: manifest.info.name,
                    online: nil
                )
                await MainActor.run {
                    urlParsing = false
                    showUrlModal = false
                    urlInput = ""
                    onOpenRoom(card)
                }
            } catch {
                await MainActor.run {
                    urlParsing = false
                    appState.showToast(error.localizedDescription)
                }
            }
        }
    }
}
