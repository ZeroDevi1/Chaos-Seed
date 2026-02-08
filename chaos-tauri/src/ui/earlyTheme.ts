import { getThemeMode, type ThemeMode } from '@/shared/prefs'
import { applyFluentTokens, type ResolvedTheme } from './fluent'

function prefersDark(): boolean {
  try {
    return typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)')?.matches
  } catch {
    return false
  }
}

function resolve(mode: ThemeMode, sysDark: boolean): ResolvedTheme {
  if (mode === 'light') return 'light'
  if (mode === 'dark') return 'dark'
  return sysDark ? 'dark' : 'light'
}

/**
 * Apply theme before Vue mounts, to avoid a white flash / missing CSS variables in secondary windows.
 */
export function applyEarlyTheme() {
  try {
    const mode = getThemeMode()
    const resolved = resolve(mode, prefersDark())
    document.documentElement.dataset.theme = resolved
    applyFluentTokens(resolved)
  } catch {
    // If storage access is blocked/unavailable in a secondary webview, avoid aborting app mount.
  }
}
