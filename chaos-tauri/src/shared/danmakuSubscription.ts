export type DanmakuSubscriptionInput = {
  routePath: string
  chatOpen: boolean
  overlayOpen: boolean
}

/**
 * Decide whether the main window should subscribe to high-frequency danmaku messages.
 *
 * We only subscribe when the user is actively on the danmaku page AND no auxiliary renderer window is open.
 * (Chat/Overlay become the primary renderers to reduce DOM/JS pressure on the main window).
 */
export function shouldSubscribeMainDanmaku(input: DanmakuSubscriptionInput): boolean {
  return input.routePath === '/danmaku' && !input.chatOpen && !input.overlayOpen
}
