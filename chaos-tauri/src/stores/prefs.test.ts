import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

const KEY_THEME = 'chaos_seed_theme_mode'
const KEY_SIDEBAR = 'chaos_seed_sidebar_collapsed'
const KEY_OVERLAY = 'chaos_seed_overlay_mode'
const KEY_BACKDROP = 'chaos_seed_backdrop_mode'

describe('prefs store', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('loads persisted values from localStorage', async () => {
    localStorage.setItem(KEY_THEME, 'dark')
    localStorage.setItem(KEY_SIDEBAR, '1')
    localStorage.setItem(KEY_OVERLAY, 'opaque')
    localStorage.setItem(KEY_BACKDROP, 'none')

    vi.resetModules()
    const { prefs, resolvedTheme } = await import('./prefs')

    const s = get(prefs)
    expect(s.themeMode).toBe('dark')
    expect(s.sidebarCollapsed).toBe(true)
    expect(s.overlayMode).toBe('opaque')
    expect(s.backdropMode).toBe('none')
    expect(get(resolvedTheme)).toBe('dark')
  })

  it('persists theme mode changes', async () => {
    vi.resetModules()
    const { prefs, resolvedTheme } = await import('./prefs')

    prefs.setThemeMode('light')
    expect(localStorage.getItem(KEY_THEME)).toBe('light')
    expect(get(resolvedTheme)).toBe('light')
  })

  it('persists overlay, sidebar, and backdrop preferences', async () => {
    vi.resetModules()
    const { prefs } = await import('./prefs')

    prefs.setOverlayMode('opaque')
    prefs.setSidebarCollapsed(true)
    prefs.setBackdropMode('mica')

    expect(localStorage.getItem(KEY_OVERLAY)).toBe('opaque')
    expect(localStorage.getItem(KEY_SIDEBAR)).toBe('1')
    expect(localStorage.getItem(KEY_BACKDROP)).toBe('mica')
  })

  it('syncs changes across windows (storage + polling)', async () => {
    vi.useFakeTimers()
    try {
      vi.resetModules()
      const { prefs } = await import('./prefs')

      const stop = prefs.startCrossWindowSync()

      // Simulate another window updating localStorage.
      localStorage.setItem(KEY_THEME, 'dark')
      window.dispatchEvent(new StorageEvent('storage', { key: KEY_THEME, newValue: 'dark' }))

      await vi.advanceTimersByTimeAsync(300)
      expect(get(prefs).themeMode).toBe('dark')

      stop()
    } finally {
      vi.useRealTimers()
    }
  })

  it('defaults backdropMode based on platform when not persisted', async () => {
    // jsdom userAgent should not match Windows, so default is 'none'.
    vi.resetModules()
    const { prefs: p1 } = await import('./prefs')
    expect(get(p1).backdropMode).toBe('none')

    // Simulate Windows user agent.
    Object.defineProperty(window.navigator, 'userAgent', {
      value: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
      configurable: true
    })

    localStorage.clear()
    vi.resetModules()
    const { prefs: p2 } = await import('./prefs')
    expect(get(p2).backdropMode).toBe('mica')
  })
})

