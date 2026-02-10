export type BootView =
  | 'main'
  | 'chat'
  | 'overlay'
  | 'player'
  | 'lyrics_chat'
  | 'lyrics_overlay'
  | 'lyrics_dock'
  | 'lyrics_float'

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
  if (view === 'lyrics_chat') return 'lyrics_chat'
  if (view === 'lyrics_overlay') return 'lyrics_overlay'
  if (view === 'lyrics_dock') return 'lyrics_dock'
  if (view === 'lyrics_float') return 'lyrics_float'
  if (view === 'main') return 'main'
  return null
}

function viewFromBoot(boot: unknown): BootView | null {
  if (!boot || typeof boot !== 'object') return null
  const v = (boot as { view?: unknown }).view
  if (
    v === 'chat' ||
    v === 'overlay' ||
    v === 'player' ||
    v === 'main' ||
    v === 'lyrics_chat' ||
    v === 'lyrics_overlay' ||
    v === 'lyrics_dock' ||
    v === 'lyrics_float'
  )
    return v
  return null
}

function viewFromLabel(label: string | null | undefined): BootView | null {
  const l = normalizeLabel(label)
  if (l === 'chat') return 'chat'
  if (l === 'overlay') return 'overlay'
  if (l === 'player') return 'player'
  if (l === 'lyrics_chat') return 'lyrics_chat'
  if (l === 'lyrics_overlay') return 'lyrics_overlay'
  if (l === 'lyrics_dock') return 'lyrics_dock'
  if (l === 'lyrics_float') return 'lyrics_float'
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
