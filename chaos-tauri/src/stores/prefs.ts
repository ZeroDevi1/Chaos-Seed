import { defineStore } from 'pinia'

import {
  getOverlayMode,
  getSidebarCollapsed,
  getThemeMode,
  setOverlayMode,
  setSidebarCollapsed,
  setThemeMode,
  type OverlayMode,
  type ThemeMode
} from '../shared/prefs'

export type ResolvedTheme = 'light' | 'dark'

function safeMatchMedia(query: string): MediaQueryList | null {
  if (typeof window === 'undefined') return null
  if (typeof window.matchMedia !== 'function') return null
  return window.matchMedia(query)
}

export const usePrefsStore = defineStore('prefs', {
  state: () => {
    const mql = safeMatchMedia('(prefers-color-scheme: dark)')
    return {
      themeMode: getThemeMode() as ThemeMode,
      sidebarCollapsed: getSidebarCollapsed(),
      overlayMode: getOverlayMode() as OverlayMode,
      prefersDark: mql?.matches ?? false
    }
  },
  getters: {
    resolvedTheme(state): ResolvedTheme {
      if (state.themeMode === 'light') return 'light'
      if (state.themeMode === 'dark') return 'dark'
      return state.prefersDark ? 'dark' : 'light'
    }
  },
  actions: {
    setThemeMode(mode: ThemeMode) {
      if (mode === this.themeMode) return
      this.themeMode = mode
      setThemeMode(mode)
    },
    setSidebarCollapsed(v: boolean) {
      if (v === this.sidebarCollapsed) return
      this.sidebarCollapsed = v
      setSidebarCollapsed(v)
    },
    setOverlayMode(mode: OverlayMode) {
      if (mode === this.overlayMode) return
      this.overlayMode = mode
      setOverlayMode(mode)
    },
    startSystemThemeSync() {
      const mql = safeMatchMedia('(prefers-color-scheme: dark)')
      if (!mql) return () => {}

      const update = () => {
        // Always track system theme; resolvedTheme decides whether to use it.
        this.prefersDark = mql.matches
      }
      update()

      // Safari historically supports addListener/removeListener.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const anyMql: any = mql
      if (typeof mql.addEventListener === 'function') {
        mql.addEventListener('change', update)
        return () => mql.removeEventListener('change', update)
      }
      if (typeof anyMql.addListener === 'function') {
        anyMql.addListener(update)
        return () => anyMql.removeListener(update)
      }
      return () => {}
    },
    /**
     * Keep preferences in sync across multiple Tauri windows.
     *
     * NOTE: On some platforms/webviews, `storage` events across webviews may be unreliable.
     * We combine `storage` with a low-frequency poll for robustness.
     */
    startCrossWindowSync() {
      let stopped = false

      const sync = () => {
        if (stopped) return
        const t = getThemeMode() as ThemeMode
        const s = getSidebarCollapsed()
        const o = getOverlayMode() as OverlayMode
        if (t !== this.themeMode) this.themeMode = t
        if (s !== this.sidebarCollapsed) this.sidebarCollapsed = s
        if (o !== this.overlayMode) this.overlayMode = o
      }

      const onStorage = (ev: StorageEvent) => {
        // Any prefs key change triggers a sync.
        if (!ev.key) return
        if (
          ev.key === 'chaos_seed_theme_mode' ||
          ev.key === 'chaos_seed_sidebar_collapsed' ||
          ev.key === 'chaos_seed_overlay_mode'
        ) {
          sync()
        }
      }

      window.addEventListener('storage', onStorage)
      // Best-effort polling: cheap and avoids "only updates after resize" on some WebView2 setups.
      const timer = window.setInterval(sync, 250)
      sync()

      return () => {
        stopped = true
        window.removeEventListener('storage', onStorage)
        window.clearInterval(timer)
      }
    }
  }
})
