import { derived, get, writable } from 'svelte/store'

import {
  getBackdropMode,
  getOverlayMode,
  getSidebarCollapsed,
  getThemeMode,
  setBackdropMode,
  setOverlayMode,
  setSidebarCollapsed,
  setThemeMode,
  type BackdropMode,
  type OverlayMode,
  type ThemeMode
} from '../shared/prefs'

export type ResolvedTheme = 'light' | 'dark'

type PrefsState = {
  themeMode: ThemeMode
  sidebarCollapsed: boolean
  overlayMode: OverlayMode
  backdropMode: BackdropMode
  prefersDark: boolean
}

function safeMatchMedia(query: string): MediaQueryList | null {
  if (typeof window === 'undefined') return null
  if (typeof window.matchMedia !== 'function') return null
  return window.matchMedia(query)
}

function readFromStorage(): Pick<PrefsState, 'themeMode' | 'sidebarCollapsed' | 'overlayMode' | 'backdropMode'> {
  return {
    themeMode: getThemeMode(),
    sidebarCollapsed: getSidebarCollapsed(),
    overlayMode: getOverlayMode(),
    backdropMode: getBackdropMode()
  }
}

const mql = safeMatchMedia('(prefers-color-scheme: dark)')

const state = writable<PrefsState>({
  ...readFromStorage(),
  prefersDark: mql?.matches ?? false
})

export const resolvedTheme = derived(state, (s): ResolvedTheme => {
  if (s.themeMode === 'light') return 'light'
  if (s.themeMode === 'dark') return 'dark'
  return s.prefersDark ? 'dark' : 'light'
})

function updateStorageBacked<K extends keyof Pick<
  PrefsState,
  'themeMode' | 'sidebarCollapsed' | 'overlayMode' | 'backdropMode'
>>(key: K, value: PrefsState[K]) {
  state.update((s) => ({ ...s, [key]: value }))
}

function startSystemThemeSync(): () => void {
  const mq = safeMatchMedia('(prefers-color-scheme: dark)')
  if (!mq) return () => {}

  const update = () => {
    state.update((s) => ({ ...s, prefersDark: mq.matches }))
  }
  update()

  // Safari historically supports addListener/removeListener.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const anyMql: any = mq
  if (typeof mq.addEventListener === 'function') {
    mq.addEventListener('change', update)
    return () => mq.removeEventListener('change', update)
  }
  if (typeof anyMql.addListener === 'function') {
    anyMql.addListener(update)
    return () => anyMql.removeListener(update)
  }
  return () => {}
}

/**
 * Keep preferences in sync across multiple Tauri windows.
 *
 * NOTE: On some platforms/webviews, `storage` events across webviews may be unreliable.
 * We combine `storage` with a low-frequency poll for robustness.
 */
function startCrossWindowSync(): () => void {
  let stopped = false

  const sync = () => {
    if (stopped) return
    const next = readFromStorage()
    state.update((s) => {
      // Always keep prefersDark from the live matchMedia sync.
      let out = s
      if (next.themeMode !== s.themeMode) out = { ...out, themeMode: next.themeMode }
      if (next.sidebarCollapsed !== s.sidebarCollapsed) out = { ...out, sidebarCollapsed: next.sidebarCollapsed }
      if (next.overlayMode !== s.overlayMode) out = { ...out, overlayMode: next.overlayMode }
      if (next.backdropMode !== s.backdropMode) out = { ...out, backdropMode: next.backdropMode }
      return out
    })
  }

  const onStorage = (ev: StorageEvent) => {
    if (!ev.key) return
    if (
      ev.key === 'chaos_seed_theme_mode' ||
      ev.key === 'chaos_seed_sidebar_collapsed' ||
      ev.key === 'chaos_seed_overlay_mode' ||
      ev.key === 'chaos_seed_backdrop_mode'
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

export const prefs = {
  subscribe: state.subscribe,

  setThemeMode(mode: ThemeMode) {
    if (mode === get(state).themeMode) return
    setThemeMode(mode)
    updateStorageBacked('themeMode', mode)
  },

  setSidebarCollapsed(v: boolean) {
    if (v === get(state).sidebarCollapsed) return
    setSidebarCollapsed(v)
    updateStorageBacked('sidebarCollapsed', v)
  },

  setOverlayMode(mode: OverlayMode) {
    if (mode === get(state).overlayMode) return
    setOverlayMode(mode)
    updateStorageBacked('overlayMode', mode)
  },

  setBackdropMode(mode: BackdropMode) {
    if (mode === get(state).backdropMode) return
    setBackdropMode(mode)
    updateStorageBacked('backdropMode', mode)
  },

  startSystemThemeSync,
  startCrossWindowSync
}
