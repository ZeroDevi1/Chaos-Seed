export type ThemeMode = 'system' | 'light' | 'dark'
export type OverlayMode = 'transparent' | 'opaque'

const KEYS = {
  themeMode: 'chaos_seed_theme_mode',
  sidebarCollapsed: 'chaos_seed_sidebar_collapsed',
  overlayMode: 'chaos_seed_overlay_mode'
} as const

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
  return 'transparent'
}

export function setOverlayMode(v: OverlayMode) {
  localStorage.setItem(KEYS.overlayMode, v)
}

