import AppKit
import CoreImage
import CoreImage.CIFilterBuiltins
import SwiftUI

@MainActor
final class BiliAccountSettingsViewModel: ObservableObject {
    @Published var account: BiliAccount?
    @Published var hasCookie = false
    @Published var loading = false
    @Published var errorMessage: String?

    private let client: BiliAccountClient

    init(client: BiliAccountClient = BiliAccountClient()) {
        self.client = client
        self.account = BiliAccountStore.loadAccount()
        self.hasCookie = BiliAccountStore.loadCookie() != nil
    }

    var isLoggedIn: Bool {
        hasCookie
    }

    var subtitle: String {
        if let account {
            return "已登录：\(account.name)（UID \(account.uid)）"
        }
        if let errorMessage {
            return "Cookie 已保存，但账号校验失败：\(errorMessage)"
        }
        if hasCookie {
            return "Cookie 已保存，将用于 Bilibili 取流。"
        }
        return "登录后请求播放地址会携带账号 Cookie。"
    }

    func refresh() async {
        hasCookie = BiliAccountStore.loadCookie() != nil
        account = BiliAccountStore.loadAccount()
        errorMessage = nil
        guard let cookie = BiliAccountStore.loadCookie() else { return }
        loading = true
        defer { loading = false }
        do {
            let fetched = try await client.fetchAccount(cookie: cookie)
            BiliAccountStore.saveAccount(fetched)
            account = fetched
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func logout() {
        do {
            try BiliAccountStore.deleteCookie()
        } catch {
            errorMessage = error.localizedDescription
        }
        account = nil
        hasCookie = false
    }
}

@MainActor
final class BiliQRCodeLoginViewModel: ObservableObject {
    @Published var qrImage: NSImage?
    @Published var statusText = "正在生成二维码…"
    @Published var loading = false
    @Published var canRefresh = false

    private let client: BiliAccountClient
    private var task: Task<Void, Never>?
    private var challenge: BiliQRCodeChallenge?
    private let onSuccess: () -> Void

    init(client: BiliAccountClient = BiliAccountClient(), onSuccess: @escaping () -> Void) {
        self.client = client
        self.onSuccess = onSuccess
    }

    func start() {
        task?.cancel()
        task = Task { [weak self] in
            await self?.loadAndPoll()
        }
    }

    func cancel() {
        task?.cancel()
        task = nil
    }

    func refresh() {
        start()
    }

    private func loadAndPoll() async {
        loading = true
        canRefresh = false
        statusText = "正在生成二维码…"
        qrImage = nil
        do {
            let challenge = try await client.generateQRCode()
            guard !Task.isCancelled else { return }
            self.challenge = challenge
            qrImage = QRCodeImage.make(from: challenge.url, size: 220)
            statusText = "请使用哔哩哔哩手机客户端扫码"
            loading = false
            try await poll(challenge: challenge)
        } catch is CancellationError {
            return
        } catch {
            loading = false
            canRefresh = true
            statusText = "二维码加载失败：\(error.localizedDescription)"
        }
    }

    private func poll(challenge: BiliQRCodeChallenge) async throws {
        while !Task.isCancelled {
            try await Task.sleep(nanoseconds: 3_000_000_000)
            let state = try await client.pollQRCode(key: challenge.key)
            guard !Task.isCancelled else { return }
            switch state {
            case .waiting:
                statusText = "等待扫码…"
            case .scanned:
                statusText = "已扫码，请在手机上确认"
            case .expired:
                canRefresh = true
                statusText = "二维码已失效，请刷新"
                return
            case .success(let cookie):
                try BiliAccountStore.saveCookie(cookie)
                let account = try? await client.fetchAccount(cookie: cookie)
                if let account {
                    BiliAccountStore.saveAccount(account)
                    statusText = "登录成功：\(account.name)"
                } else {
                    statusText = "登录成功"
                }
                onSuccess()
                return
            }
        }
    }
}

struct BiliQRCodeLoginSheet: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var vm: BiliQRCodeLoginViewModel

    init(onSuccess: @escaping () -> Void) {
        _vm = StateObject(wrappedValue: BiliQRCodeLoginViewModel(onSuccess: onSuccess))
    }

    var body: some View {
        VStack(spacing: 18) {
            HStack {
                Text("Bilibili 扫码登录")
                    .font(.system(size: 16, weight: .bold))
                Spacer()
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.borderless)
            }

            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(Color.white)
                    .frame(width: 244, height: 244)
                if let qrImage = vm.qrImage {
                    Image(nsImage: qrImage)
                        .interpolation(.none)
                        .resizable()
                        .frame(width: 220, height: 220)
                } else {
                    ProgressView()
                        .controlSize(.regular)
                }
            }

            Text(vm.statusText)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(minHeight: 20)

            HStack {
                Button("刷新二维码") {
                    vm.refresh()
                }
                .disabled(vm.loading && !vm.canRefresh)

                Spacer()

                Button("关闭") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)
            }
        }
        .padding(22)
        .frame(width: 360)
        .onAppear { vm.start() }
        .onDisappear { vm.cancel() }
    }
}

private enum QRCodeImage {
    static func make(from string: String, size: CGFloat) -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scale = size / output.extent.width
        let image = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        let context = CIContext()
        guard let cgImage = context.createCGImage(image, from: image.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: size, height: size))
    }
}
