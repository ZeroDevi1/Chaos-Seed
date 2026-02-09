import type { LiveManifest, StreamVariant } from './livestreamTypes'

function hasUrl(v: StreamVariant): boolean {
  const u = (v.url ?? '').toString().trim()
  return u.length > 0
}

export function pickDefaultVariant(manifest: LiveManifest | null | undefined): StreamVariant | null {
  if (!manifest) return null
  const vars = manifest.variants || []
  if (vars.length === 0) return null
  const withUrl = vars.filter(hasUrl)
  if (withUrl.length > 0) {
    // Highest quality first.
    return withUrl.reduce((best, cur) => (cur.quality > best.quality ? cur : best), withUrl[0])
  }
  return vars[0] ?? null
}

export function mergeVariant(manifest: LiveManifest, resolved: StreamVariant): LiveManifest {
  const idx = (manifest.variants || []).findIndex((v) => v.id === resolved.id)
  if (idx < 0) return manifest
  const next = [...manifest.variants]
  next[idx] = { ...next[idx], ...resolved }
  return { ...manifest, variants: next }
}

