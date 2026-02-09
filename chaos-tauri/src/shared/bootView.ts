export type BootView = 'main' | 'chat' | 'overlay' | 'player'

export type ResolveViewInput = {
  boot: unknown
  search: string
  label?: string | null | undefined
}

function normalizeLabel(label: string | null | undefined): string {
  return (label || '').toString().trim().toLowerCase()
}

function viewFromQuery(search: string): BootView | null {
  const params = new URLSearchParams(search || '')
  const view = (params.get('view') || '').toLowerCase()
  if (view === 'chat') return 'chat'
  if (view === 'overlay') return 'overlay'
  if (view === 'player') return 'player'
  if (view === 'main') return 'main'
  return null
}

function viewFromBoot(boot: unknown): BootView | null {
  if (!boot || typeof boot !== 'object') return null
  const v = (boot as { view?: unknown }).view
  if (v === 'chat' || v === 'overlay' || v === 'player' || v === 'main') return v
  return null
}

function viewFromLabel(label: string | null | undefined): BootView | null {
  const l = normalizeLabel(label)
  if (l === 'chat') return 'chat'
  if (l === 'overlay') return 'overlay'
  if (l === 'player') return 'player'
  if (l === 'main') return 'main'
  return null
}

/**
 * Determines which UI entrypoint should mount for the current window.
 *
 * Priority:
 * 1) `boot.view` injected by Tauri initialization_script (multi-window safe)
 * 2) `?view=...` query param (dev-friendly fallback)
 * 3) current webview window label (`chat`/`overlay`)
 * 4) default to `main`
 */
export function resolveView(input: ResolveViewInput): BootView {
  return (
    viewFromBoot(input.boot) ??
    viewFromQuery(input.search) ??
    viewFromLabel(input.label) ??
    'main'
  )
}

// Back-compat: older call sites.
export function getBootView(boot: unknown, search: string): BootView {
  return resolveView({ boot, search })
}
