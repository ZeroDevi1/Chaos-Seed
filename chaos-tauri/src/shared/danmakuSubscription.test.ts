import { describe, expect, it } from 'vitest'

import { shouldSubscribeMainDanmaku } from './danmakuSubscription'

describe('danmaku subscription', () => {
  it('subscribes only when on /danmaku and chat is closed', () => {
    expect(shouldSubscribeMainDanmaku({ routePath: '/danmaku', chatOpen: false })).toBe(true)
    expect(shouldSubscribeMainDanmaku({ routePath: '/danmaku', chatOpen: true })).toBe(false)
    expect(shouldSubscribeMainDanmaku({ routePath: '/', chatOpen: false })).toBe(false)
    expect(shouldSubscribeMainDanmaku({ routePath: '/settings', chatOpen: false })).toBe(false)
  })
})

