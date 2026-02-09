import { getThemeMode, setThemeMode, type ThemeMode } from '../shared/prefs'

export type ResolvedTheme = 'light' | 'dark'

export type ThemeController = {
  getMode(): ThemeMode
  setMode(mode: ThemeMode): void
  getResolved(): ResolvedTheme
  onChange(cb: (mode: ThemeMode, resolved: ResolvedTheme) => void): () => void
}

function resolve(mode: ThemeMode, prefersDark: boolean): ResolvedTheme {
  if (mode === 'dark') return 'dark'
  if (mode === 'light') return 'light'
  return prefersDark ? 'dark' : 'light'
}

function applyResolved(resolved: ResolvedTheme) {
  document.documentElement.dataset.theme = resolved
}

export function initTheme(): ThemeController {
  let mode: ThemeMode = getThemeMode()
  const mql = window.matchMedia?.('(prefers-color-scheme: dark)')
  let prefersDark = mql?.matches ?? false
  let resolved: ResolvedTheme = resolve(mode, prefersDark)

  applyResolved(resolved)

  const listeners = new Set<(mode: ThemeMode, resolved: ResolvedTheme) => void>()

  function notify() {
    for (const cb of listeners) cb(mode, resolved)
  }

  function applyIfNeeded(): boolean {
    const next = resolve(mode, prefersDark)
    if (next === resolved) return false
    resolved = next
    applyResolved(resolved)
    return true
  }

  const onMqlChange = () => {
    prefersDark = mql?.matches ?? false
    if (mode === 'system') {
      if (applyIfNeeded()) notify()
    }
  }

  if (mql) {
    // Safari uses addListener/removeListener historically; keep both.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const anyMql: any = mql
    if (typeof mql.addEventListener === 'function') mql.addEventListener('change', onMqlChange)
    else if (typeof anyMql.addListener === 'function') anyMql.addListener(onMqlChange)
  }

  function setMode(mode2: ThemeMode) {
    if (mode2 === mode) return
    mode = mode2
    setThemeMode(mode2)
    applyIfNeeded()
    notify()
  }

  return {
    getMode: () => mode,
    setMode,
    getResolved: () => resolved,
    onChange: (cb) => {
      listeners.add(cb)
      cb(mode, resolved)
      return () => listeners.delete(cb)
    }
  }
}
