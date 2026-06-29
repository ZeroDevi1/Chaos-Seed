import AppKit
import CMpv
import CPrivatePIP
import Darwin
import Foundation
import OpenGL.GL3
import SwiftUI

/// 内嵌 libmpv 播放核心，负责 HTTP-FLV 等 AVFoundation 不支持的封装。
///
/// 渲染走 mpv render API + AppKit OpenGL 视图；控制层仍由 SwiftUI HUD 统一管理。
final class LibMPVPlayerCore {
    typealias PlaybackCallback = (String?) -> Void

    private var handle: OpaquePointer?
    private var renderContext: OpaquePointer?
    private weak var view: LibMPVOpenGLView?
    private var pendingURL: URL?
    private var pendingHints = PlaybackHints()
    private var isInitialized = false
    private let onPlaybackStarted: PlaybackCallback
    private let onPlaybackFailed: PlaybackCallback

    init(
        onPlaybackStarted: @escaping PlaybackCallback,
        onPlaybackFailed: @escaping PlaybackCallback
    ) {
        self.onPlaybackStarted = onPlaybackStarted
        self.onPlaybackFailed = onPlaybackFailed
    }

    deinit {
        destroy()
    }

    func play(url: URL, hints: PlaybackHints) -> String? {
        pendingURL = url
        pendingHints = hints
        if let error = ensureHandle(hints: hints) {
            return error
        }
        return loadPendingIfReady()
    }

    func stop() {
        _ = command(["stop"])
        pendingURL = nil
    }

    func togglePlayback(isPlaying: Bool) {
        _ = setPause(isPlaying)
    }

    func setVolume(_ volume: Float) {
        let percent = Int((min(max(volume, 0), 1) * 100).rounded())
        _ = command(["set", "volume", "\(percent)"])
    }

    func setMuted(_ muted: Bool) {
        _ = command(["set", "mute", muted ? "yes" : "no"])
    }

    func attach(view: LibMPVOpenGLView) {
        self.view = view
    }

    func prepareRenderContext(for view: LibMPVOpenGLView) -> String? {
        attach(view: view)
        guard renderContext == nil else {
            return loadPendingIfReady()
        }
        guard let handle else {
            return "libmpv 尚未初始化"
        }

        var glInit = mpv_opengl_init_params(
            get_proc_address: libMPVGetOpenGLProcAddress,
            get_proc_address_ctx: nil
        )
        var context: OpaquePointer?
        let status = MPV_RENDER_API_TYPE_OPENGL.withCString { apiType in
            withUnsafeMutablePointer(to: &glInit) { glInitPointer in
                var params = [
                    mpv_render_param(
                        type: MPV_RENDER_PARAM_API_TYPE,
                        data: UnsafeMutableRawPointer(mutating: apiType)
                    ),
                    mpv_render_param(
                        type: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                        data: UnsafeMutableRawPointer(glInitPointer)
                    ),
                    mpv_render_param()
                ]
                return mpv_render_context_create(&context, handle, &params)
            }
        }
        guard status >= 0, let context else {
            return mpvError(status, fallback: "无法创建 libmpv 渲染上下文")
        }
        renderContext = context

        let viewPointer = Unmanaged.passUnretained(view).toOpaque()
        mpv_render_context_set_update_callback(
            context,
            libMPVRenderUpdateCallback,
            viewPointer
        )

        return loadPendingIfReady()
    }

    func render(width: Int32, height: Int32) {
        guard let renderContext else { return }
        drainEvents()

        var fbo = mpv_opengl_fbo(
            fbo: 0,
            w: width,
            h: height,
            internal_format: GL_RGBA8
        )
        var flip: Int32 = 1
        withUnsafeMutablePointer(to: &fbo) { fboPointer in
            withUnsafeMutablePointer(to: &flip) { flipPointer in
                var params = [
                    mpv_render_param(
                        type: MPV_RENDER_PARAM_OPENGL_FBO,
                        data: UnsafeMutableRawPointer(fboPointer)
                    ),
                    mpv_render_param(
                        type: MPV_RENDER_PARAM_FLIP_Y,
                        data: UnsafeMutableRawPointer(flipPointer)
                    ),
                    mpv_render_param()
                ]
                mpv_render_context_render(renderContext, &params)
            }
        }
    }

    func destroyRenderContext() {
        guard let renderContext else { return }
        mpv_render_context_set_update_callback(renderContext, nil, nil)
        mpv_render_context_free(renderContext)
        self.renderContext = nil
    }

    func destroy() {
        destroyRenderContext()
        if let handle {
            mpv_terminate_destroy(handle)
        }
        handle = nil
        isInitialized = false
    }

    private func ensureHandle(hints: PlaybackHints) -> String? {
        if handle != nil {
            applyNetworkHints(hints)
            return nil
        }

        guard let created = mpv_create() else {
            return "无法创建 libmpv 实例，请确认 Homebrew mpv 已安装"
        }
        handle = created

        setOption("terminal", "no")
        setOption("msg-level", "all=warn")
        setOption("vo", "libmpv")
        setOption("idle", "yes")
        setOption("ytdl", "no")
        setOption("hwdec", "auto-safe")
        setOption("stream-lavf-o", "reconnect_streamed=yes")
        applyNetworkHints(hints)

        let status = mpv_initialize(created)
        guard status >= 0 else {
            let message = mpvError(status, fallback: "libmpv 初始化失败")
            destroy()
            return message
        }
        isInitialized = true
        return nil
    }

    private func applyNetworkHints(_ hints: PlaybackHints) {
        if let referer = hints.referer {
            setOption("referrer", referer)
        }
        if let userAgent = hints.userAgent {
            setOption("user-agent", userAgent)
        }
    }

    private func loadPendingIfReady() -> String? {
        guard isInitialized, renderContext != nil, let pendingURL else {
            return nil
        }
        let status = command(["loadfile", pendingURL.absoluteString, "replace"])
        if status >= 0 {
            self.pendingURL = nil
            onPlaybackStarted(nil)
            return nil
        }
        let message = mpvError(status, fallback: "libmpv 打开直播流失败")
        onPlaybackFailed(message)
        return message
    }

    private func setPause(_ paused: Bool) -> Int32 {
        command(["set", "pause", paused ? "yes" : "no"])
    }

    private func setOption(_ name: String, _ value: String) {
        guard let handle else { return }
        mpv_set_option_string(handle, name, value)
    }

    @discardableResult
    private func command(_ args: [String]) -> Int32 {
        guard let handle else { return MPV_ERROR_UNINITIALIZED.rawValue }
        let cStrings = args.map { strdup($0) }
        defer {
            for pointer in cStrings {
                free(pointer)
            }
        }
        var argv = cStrings.map { UnsafePointer<CChar>($0) }
        argv.append(nil)
        return mpv_command(handle, &argv)
    }

    private func drainEvents() {
        guard let handle else { return }
        while true {
            guard let event = mpv_wait_event(handle, 0) else { return }
            switch event.pointee.event_id {
            case MPV_EVENT_NONE:
                return
            case MPV_EVENT_SHUTDOWN:
                return
            case MPV_EVENT_END_FILE:
                onPlaybackFailed("libmpv 播放结束或当前线路中断")
            default:
                continue
            }
        }
    }

    private func mpvError(_ code: Int32, fallback: String) -> String {
        guard let pointer = mpv_error_string(code) else {
            return fallback
        }
        return "\(fallback)：\(String(cString: pointer))"
    }
}

struct LibMPVPlayerView: NSViewRepresentable {
    @ObservedObject var player: BuiltinPlayer

    func makeNSView(context: Context) -> LibMPVContainerView {
        guard let videoView = LibMPVOpenGLView(player: player) else {
            preconditionFailure("无法创建 libmpv OpenGL 渲染视图")
        }
        let container = LibMPVContainerView(videoView: videoView)
        player.attachLibMPVView(videoView)
        return container
    }

    func updateNSView(_ nsView: LibMPVContainerView, context: Context) {
        nsView.videoView.player = player
        player.attachLibMPVView(nsView.videoView)
    }

    static func dismantleNSView(_ nsView: LibMPVContainerView, coordinator: ()) {
        nsView.videoView.player?.detachLibMPVView(nsView.videoView)
        nsView.videoView.player = nil
    }
}

final class LibMPVContainerView: NSView {
    let videoView: LibMPVOpenGLView

    init(videoView: LibMPVOpenGLView) {
        self.videoView = videoView
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        addSubview(videoView)
    }

    required init?(coder: NSCoder) {
        nil
    }

    override func layout() {
        super.layout()
        if videoView.superview === self {
            videoView.frame = bounds
        }
    }

    func restoreVideoViewIfNeeded() {
        guard videoView.superview !== self else { return }
        addSubview(videoView)
        videoView.frame = bounds
        videoView.needsDisplay = true
    }
}

final class LibMPVOpenGLView: NSOpenGLView {
    weak var player: BuiltinPlayer?

    init?(player: BuiltinPlayer) {
        guard let pixelFormat = Self.makePixelFormat() else {
            return nil
        }
        super.init(frame: .zero, pixelFormat: pixelFormat)
        self.player = player
        wantsBestResolutionOpenGLSurface = true
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        pixelFormat = Self.makePixelFormat()
        wantsBestResolutionOpenGLSurface = true
    }

    override func prepareOpenGL() {
        super.prepareOpenGL()
        openGLContext?.makeCurrentContext()
        if let message = player?.prepareLibMPVRenderContext(for: self) {
            player?.libMPVDidFail(message)
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = openGLContext else { return }
        context.makeCurrentContext()
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
        let width = max(1, Int32(bounds.width * scale))
        let height = max(1, Int32(bounds.height * scale))
        glViewport(0, 0, width, height)
        player?.renderLibMPV(width: width, height: height)
        context.flushBuffer()
    }

    override func reshape() {
        super.reshape()
        needsDisplay = true
    }

    private static func makePixelFormat() -> NSOpenGLPixelFormat? {
        var attributes: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAOpenGLProfile),
            UInt32(NSOpenGLProfileVersion3_2Core),
            UInt32(NSOpenGLPFAAccelerated),
            UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAColorSize),
            24,
            UInt32(NSOpenGLPFAAlphaSize),
            8,
            UInt32(NSOpenGLPFADepthSize),
            0,
            0,
        ]
        return NSOpenGLPixelFormat(attributes: &attributes)
    }
}

private let libMPVGetOpenGLProcAddress: @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafePointer<CChar>?
) -> UnsafeMutableRawPointer? = { _, name in
    guard let name else { return nil }
    return dlsym(libMPVOpenGLHandle(), name)
}

private let libMPVRenderUpdateCallback: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { context in
    guard let context else { return }
    let view = Unmanaged<LibMPVOpenGLView>
        .fromOpaque(context)
        .takeUnretainedValue()
    DispatchQueue.main.async { [weak view] in
        view?.needsDisplay = true
    }
}

private func libMPVOpenGLHandle() -> UnsafeMutableRawPointer? {
    struct Holder {
        static let handle = dlopen(
            "/System/Library/Frameworks/OpenGL.framework/OpenGL",
            RTLD_LAZY | RTLD_LOCAL
        )
    }
    return Holder.handle
}
