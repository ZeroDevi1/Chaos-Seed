import type { LyricsSearchResult } from './types'

export type ParsedLrcRow = {
  kind: 'meta' | 'line'
  timeMs?: number
  text: string
}

export type DisplayRow = {
  original: string
  translation?: string
  isMeta: boolean
}

function parseTimeMs(tag: string): number | null {
  // tag is like "mm:ss.xx" or "mm:ss"
  const m = tag.match(/^(\d+):(\d+(?:\.\d+)?)$/)
  if (!m) return null
  const min = Number(m[1])
  const sec = Number(m[2])
  if (!Number.isFinite(min) || !Number.isFinite(sec)) return null
  const ms = Math.round((min * 60 + sec) * 1000)
  return ms >= 0 ? ms : null
}

export function parseLrc(text: string): ParsedLrcRow[] {
  const s = (text ?? '').toString()
  if (!s.trim()) return []

  const out: ParsedLrcRow[] = []
  for (const rawLine of s.split(/\r?\n/)) {
    const line = rawLine.replace(/\r/g, '')
    if (!line.trim()) continue

    // metadata line: [ti:...], [ar:...], etc. (but not a time tag)
    const meta = line.match(/^\[([a-zA-Z]+):(.*)\]$/)
    const time = line.match(/^\[(\d+:\d+(?:\.\d+)?)\](.*)$/)
    if (time) {
      const timeMs = parseTimeMs(time[1])
      const content = (time[2] ?? '').toString()
      out.push({ kind: 'line', timeMs: timeMs ?? undefined, text: content.trim() })
      continue
    }
    if (meta) {
      const key = meta[1].toLowerCase()
      const val = (meta[2] ?? '').toString().trim()
      out.push({ kind: 'meta', text: `[${key}:${val}]` })
      continue
    }

    // Untagged line.
    out.push({ kind: 'line', text: line.trim() })
  }

  return out
}

export function alignTranslation(original: string, translation: string): DisplayRow[] {
  const o = parseLrc(original)
  const t = parseLrc(translation)

  const oLines = o.filter((r) => r.kind === 'line')
  const tLines = t.filter((r) => r.kind === 'line')

  const canTimeAlign = tLines.some((r) => r.timeMs !== undefined) && oLines.some((r) => r.timeMs !== undefined)

  const byTime = new Map<number, string>()
  if (canTimeAlign) {
    for (const r of tLines) {
      if (r.timeMs === undefined) continue
      const prev = byTime.get(r.timeMs)
      byTime.set(r.timeMs, prev ? `${prev}\n${r.text}` : r.text)
    }
  }

  const byIndex: string[] = []
  if (!canTimeAlign) {
    for (const r of tLines) {
      byIndex.push(r.text)
    }
  }

  const out: DisplayRow[] = []
  let idx = 0
  for (const r of o) {
    if (r.kind === 'meta') {
      out.push({ original: r.text, isMeta: true })
      continue
    }
    const tr = canTimeAlign
      ? (r.timeMs !== undefined ? byTime.get(r.timeMs) : undefined)
      : byIndex[idx]
    out.push({
      original: r.text,
      translation: tr && tr.trim() ? tr : undefined,
      isMeta: false
    })
    idx++
  }

  return out
}

export function formatForDisplay(item: LyricsSearchResult | null | undefined): DisplayRow[] {
  if (!item) return []
  const orig = (item.lyrics_original ?? '').toString()
  const trans = item.lyrics_translation ? item.lyrics_translation.toString() : ''
  if (!trans.trim()) {
    return parseLrc(orig).map((r) => ({
      original: r.text,
      isMeta: r.kind === 'meta'
    }))
  }
  return alignTranslation(orig, trans)
}

