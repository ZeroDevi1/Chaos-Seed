export type ThemeMode = 'system' | 'light' | 'dark'
export type OverlayMode = 'transparent' | 'opaque'
export type BackdropMode = 'none' | 'mica'

const KEYS = {
  themeMode: 'chaos_seed_theme_mode',
  sidebarCollapsed: 'chaos_seed_sidebar_collapsed',
  overlayMode: 'chaos_seed_overlay_mode',
  backdropMode: 'chaos_seed_backdrop_mode'
} as const

function isProbablyWindows(): boolean {
  try {
    return typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent || '')
  } catch {
    return false
  }
}

export function getThemeMode(): ThemeMode {
  const v = localStorage.getItem(KEYS.themeMode)
  if (v === 'light' || v === 'dark' || v === 'system') return v
  return 'system'
}

export function setThemeMode(v: ThemeMode) {
  localStorage.setItem(KEYS.themeMode, v)
}

export function getSidebarCollapsed(): boolean {
  return localStorage.getItem(KEYS.sidebarCollapsed) === '1'
}

export function setSidebarCollapsed(v: boolean) {
  localStorage.setItem(KEYS.sidebarCollapsed, v ? '1' : '0')
}

export function getOverlayMode(): OverlayMode {
  const v = localStorage.getItem(KEYS.overlayMode)
  if (v === 'opaque' || v === 'transparent') return v
  // Default: prefer opaque on Windows for stability (some WebView2 + transparent windows combos are flaky).
  return isProbablyWindows() ? 'opaque' : 'transparent'
}

export function setOverlayMode(v: OverlayMode) {
  localStorage.setItem(KEYS.overlayMode, v)
}

export function getBackdropMode(): BackdropMode {
  const v = localStorage.getItem(KEYS.backdropMode)
  if (v === 'none' || v === 'mica') return v
  // Default: enable Mica on Windows 11 for a more native feel; otherwise keep it disabled.
  return isProbablyWindows() ? 'mica' : 'none'
}

export function setBackdropMode(v: BackdropMode) {
  localStorage.setItem(KEYS.backdropMode, v)
}
