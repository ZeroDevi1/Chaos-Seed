import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

function makeScrollHarness(scrollHeightBase: number) {
  const scrollEl = document.createElement('div')
  const listEl = document.createElement('div')

  let scrollTop = 0
  Object.defineProperty(scrollEl, 'clientHeight', { value: 100, configurable: true })
  Object.defineProperty(scrollEl, 'scrollTop', {
    configurable: true,
    get: () => scrollTop,
    set: (v: number) => {
      scrollTop = v
    }
  })
  Object.defineProperty(scrollEl, 'scrollHeight', {
    configurable: true,
    get: () => scrollHeightBase + listEl.childElementCount * 20
  })

  return { scrollEl, listEl, getScrollTop: () => scrollTop }
}

describe('createDanmakuListStore', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('sticks to bottom when stickToBottom=true', async () => {
    const { createDanmakuListStore } = await import('./store')
    const h = makeScrollHarness(1000)

    const store = createDanmakuListStore({
      scrollEl: h.scrollEl,
      listEl: h.listEl,
      flushIntervalMs: 10,
      stickToBottom: true
    })

    store.enqueue({ site: 'bili', room_id: '1', received_at_ms: 1, user: 'u', text: 'hi' })
    await vi.advanceTimersByTimeAsync(20)

    expect(h.getScrollTop()).toBe(h.scrollEl.scrollHeight)
    store.dispose()
  })

  it('does not jump when user scrolled up and stickToBottom=false', async () => {
    const { createDanmakuListStore } = await import('./store')
    const h = makeScrollHarness(2000)
    ;(h.scrollEl as unknown as { scrollTop: number }).scrollTop = 0

    const store = createDanmakuListStore({
      scrollEl: h.scrollEl,
      listEl: h.listEl,
      flushIntervalMs: 10,
      stickToBottom: false
    })

    store.enqueue({ site: 'bili', room_id: '1', received_at_ms: 1, user: 'u', text: 'hi' })
    await vi.advanceTimersByTimeAsync(20)

    expect(h.getScrollTop()).toBe(0)
    store.dispose()
  })
})
