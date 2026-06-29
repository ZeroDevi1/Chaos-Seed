import SwiftUI
import WebKit

/// 用 WebKit Media Source Extensions 播放 AVFoundation 不支持的 HTTP-FLV。
struct WebFLVPlayerView: NSViewRepresentable {
    @ObservedObject var player: BuiltinPlayer
    let request: WebFLVPlaybackRequest

    static var resourceRootURL: URL? {
        Bundle.module.resourceURL?.appendingPathComponent("Resources", isDirectory: true)
    }

    static var playerPageURL: URL? {
        resourceRootURL?.appendingPathComponent("WebFLVPlayer.html")
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(player: player)
    }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.mediaTypesRequiringUserActionForPlayback = []
        configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        configuration.userContentController.add(context.coordinator, name: Coordinator.messageHandlerName)

        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        webView.setValue(false, forKey: "drawsBackground")
        context.coordinator.attachPlaybackActions(to: webView)
        context.coordinator.request = request
        context.coordinator.loadPlayerPage(in: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.player = player
        context.coordinator.attachPlaybackActions(to: webView)
        guard context.coordinator.request?.id != request.id else { return }
        context.coordinator.request = request
        context.coordinator.pageLoaded = false
        context.coordinator.loadPlayerPage(in: webView)
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        webView.evaluateJavaScript("destroyPlayback()")
        coordinator.player?.detachWebPlaybackActions(id: coordinator.id)
        webView.configuration.userContentController.removeScriptMessageHandler(
            forName: Coordinator.messageHandlerName
        )
        webView.navigationDelegate = nil
    }

    final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
        static let messageHandlerName = "chaosSeed"

        let id = UUID()
        weak var player: BuiltinPlayer?
        var request: WebFLVPlaybackRequest?
        var pageLoaded = false

        init(player: BuiltinPlayer) {
            self.player = player
        }

        func attachPlaybackActions(to webView: WKWebView) {
            guard let player else { return }
            player.attachWebPlaybackActions(
                id: id,
                actions: WebPlaybackActions(
                    togglePlayback: { [weak webView] in
                        webView?.evaluateJavaScript("void togglePlayback();")
                    },
                    setMuted: { [weak webView] muted in
                        webView?.evaluateJavaScript("void setMuted(\(muted));")
                    },
                    setVolume: { [weak webView] volume in
                        webView?.evaluateJavaScript("void setVolume(\(volume));")
                    },
                    togglePictureInPicture: { [weak webView, weak self] in
                        webView?.evaluateJavaScript("void togglePictureInPicture();") { _, error in
                            guard let self, let error else { return }
                            self.player?.webPictureInPictureDidFail(
                                error.localizedDescription,
                                id: self.id
                            )
                        }
                    }
                )
            )
            webView.evaluateJavaScript("void setMuted(\(player.isMuted));")
            webView.evaluateJavaScript("void setVolume(\(player.volume));")
        }

        func loadPlayerPage(in webView: WKWebView) {
            guard let bundleRoot = WebFLVPlayerView.resourceRootURL else {
                player?.webFLVDidFail("找不到内置播放器资源目录")
                return
            }
            let htmlURL = WebFLVPlayerView.playerPageURL
                ?? bundleRoot.appendingPathComponent("WebFLVPlayer.html")
            guard FileManager.default.fileExists(atPath: htmlURL.path) else {
                player?.webFLVDidFail("找不到 WebFLVPlayer.html")
                return
            }
            let scriptURL = bundleRoot.appendingPathComponent("Vendor/mpegts.js")
            guard let html = try? String(contentsOf: htmlURL, encoding: .utf8),
                  let script = try? String(contentsOf: scriptURL, encoding: .utf8),
                  let request
            else {
                player?.webFLVDidFail("无法读取 HTTP-FLV 播放器资源")
                return
            }

            // 页面与直播 CDN 使用同一 security origin，避免 file:// 页面跨源拉流失败。
            // mpegts.js 内联后不再依赖本地文件子资源访问权限。
            let inlineTag = "<script>\n\(script)\n</script>"
            let document = html.replacingOccurrences(
                of: #"<script src="Vendor/mpegts.js"></script>"#,
                with: inlineTag
            )
            webView.loadHTMLString(document, baseURL: request.url.deletingLastPathComponent())
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            pageLoaded = true
            attachPlaybackActions(to: webView)
            startPlayback(in: webView)
        }

        func webView(
            _ webView: WKWebView,
            didFail navigation: WKNavigation!,
            withError error: Error
        ) {
            player?.webFLVDidFail(error.localizedDescription)
        }

        func webView(
            _ webView: WKWebView,
            didFailProvisionalNavigation navigation: WKNavigation!,
            withError error: Error
        ) {
            player?.webFLVDidFail(error.localizedDescription)
        }

        func startPlayback(in webView: WKWebView) {
            guard let request else { return }
            let payload: [String: Any] = [
                "url": request.url.absoluteString,
                "headers": request.headers,
                "sessionId": request.id.uuidString,
            ]
            guard JSONSerialization.isValidJSONObject(payload),
                  let data = try? JSONSerialization.data(withJSONObject: payload),
                  let json = String(data: data, encoding: .utf8)
            else {
                player?.webFLVDidFail("无法构造 HTTP-FLV 播放参数")
                return
            }
            webView.evaluateJavaScript("void startPlayback(\(json));") { [weak self] _, error in
                if let error {
                    self?.player?.webFLVDidFail(error.localizedDescription)
                }
            }
        }

        func userContentController(
            _ userContentController: WKUserContentController,
            didReceive message: WKScriptMessage
        ) {
            guard message.name == Self.messageHandlerName,
                  let body = message.body as? [String: Any],
                  let type = body["type"] as? String,
                  let sessionId = body["sessionId"] as? String,
                  sessionId == request?.id.uuidString
            else {
                return
            }
            switch type {
            case "playing":
                player?.webFLVDidStartPlaying()
            case "paused":
                player?.webFLVDidPause()
            case "playbackState":
                player?.webPlaybackStateChanged(
                    isPlaying: body["playing"] as? Bool ?? false,
                    isMuted: body["muted"] as? Bool ?? false,
                    volume: Float(body["volume"] as? Double ?? 1)
                )
            case "error":
                player?.webFLVDidFail(body["message"] as? String ?? "")
            case "pictureInPictureAvailability":
                player?.webPictureInPictureAvailabilityChanged(
                    body["available"] as? Bool ?? false,
                    id: id
                )
            case "pictureInPictureState":
                player?.webPictureInPictureStateChanged(
                    body["active"] as? Bool ?? false,
                    id: id
                )
            case "pictureInPictureError":
                player?.webPictureInPictureDidFail(
                    body["message"] as? String ?? "无法切换画中画",
                    id: id
                )
            default:
                break
            }
        }
    }
}
