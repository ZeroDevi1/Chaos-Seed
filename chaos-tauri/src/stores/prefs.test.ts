import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { usePrefsStore } from './prefs'

const KEY_THEME = 'chaos_seed_theme_mode'
const KEY_SIDEBAR = 'chaos_seed_sidebar_collapsed'
const KEY_OVERLAY = 'chaos_seed_overlay_mode'

describe('prefs store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('loads persisted values from localStorage', () => {
    localStorage.setItem(KEY_THEME, 'dark')
    localStorage.setItem(KEY_SIDEBAR, '1')
    localStorage.setItem(KEY_OVERLAY, 'opaque')

    const prefs = usePrefsStore()
    expect(prefs.themeMode).toBe('dark')
    expect(prefs.sidebarCollapsed).toBe(true)
    expect(prefs.overlayMode).toBe('opaque')
    expect(prefs.resolvedTheme).toBe('dark')
  })

  it('persists theme mode changes', () => {
    const prefs = usePrefsStore()
    prefs.setThemeMode('light')
    expect(localStorage.getItem(KEY_THEME)).toBe('light')
    expect(prefs.resolvedTheme).toBe('light')
  })

  it('persists overlay and sidebar preferences', () => {
    const prefs = usePrefsStore()
    prefs.setOverlayMode('opaque')
    prefs.setSidebarCollapsed(true)
    expect(localStorage.getItem(KEY_OVERLAY)).toBe('opaque')
    expect(localStorage.getItem(KEY_SIDEBAR)).toBe('1')
  })

  it('syncs changes across windows (storage + polling)', async () => {
    vi.useFakeTimers()
    try {
      const prefs = usePrefsStore()
      expect(prefs.themeMode).toBe('system')

      const stop = prefs.startCrossWindowSync()

      // Simulate another window updating localStorage.
      localStorage.setItem(KEY_THEME, 'dark')
      window.dispatchEvent(new StorageEvent('storage', { key: KEY_THEME, newValue: 'dark' }))

      // Allow sync/poll to run.
      await vi.advanceTimersByTimeAsync(300)
      expect(prefs.themeMode).toBe('dark')

      stop()
    } finally {
      vi.useRealTimers()
    }
  })
})
