export type DanmakuSubscriptionInput = {
  routePath: string
  chatOpen: boolean
}

/**
 * Decide whether the main window should subscribe to high-frequency danmaku messages.
 *
 * We only subscribe when the user is actively on the danmaku page AND the Chat window is not open
 * (Chat becomes the primary renderer to reduce DOM/JS pressure on the main window).
 */
export function shouldSubscribeMainDanmaku(input: DanmakuSubscriptionInput): boolean {
  return input.routePath === '/danmaku' && !input.chatOpen
}

