import { describe, expect, it } from 'vitest'

import { shouldSubscribeMainDanmaku } from './danmakuSubscription'

describe('danmaku subscription', () => {
  it('subscribes only when on /danmaku and no auxiliary windows are open', () => {
    expect(shouldSubscribeMainDanmaku({ routePath: '/danmaku', chatOpen: false, overlayOpen: false })).toBe(true)
    expect(shouldSubscribeMainDanmaku({ routePath: '/danmaku', chatOpen: true, overlayOpen: false })).toBe(false)
    expect(shouldSubscribeMainDanmaku({ routePath: '/danmaku', chatOpen: false, overlayOpen: true })).toBe(false)
    expect(shouldSubscribeMainDanmaku({ routePath: '/', chatOpen: false, overlayOpen: false })).toBe(false)
    expect(shouldSubscribeMainDanmaku({ routePath: '/settings', chatOpen: false, overlayOpen: false })).toBe(false)
  })
})
