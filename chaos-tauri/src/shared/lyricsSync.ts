export type TimelineLine = {
  startMs: number
  endMs: number
  text: string
  translationText?: string | null
}

export type Timeline = {
  meta: string[]
  lines: TimelineLine[]
}

const RE_TIME_TAG = /\[(\d{1,2}):(\d{2})(?:\.(\d{1,3}))?\]/g

function parseTimeTagMs(mm: string, ss: string, frac?: string): number | null {
  const m = Number(mm)
  const s = Number(ss)
  if (!Number.isFinite(m) || !Number.isFinite(s)) return null
  if (m < 0 || s < 0 || s >= 60) return null
  let ms = 0
  if (typeof frac === 'string' && frac.length > 0) {
    const f = Number(frac)
    if (Number.isFinite(f) && f >= 0) {
      // .1 -> 100ms, .12 -> 120ms, .123 -> 123ms
      const scale = frac.length === 1 ? 100 : frac.length === 2 ? 10 : 1
      ms = f * scale
    }
  }
  return m * 60_000 + s * 1000 + ms
}

function extractTimedLines(raw: string): { meta: string[]; lines: Array<{ startMs: number; text: string }> } {
  const meta: string[] = []
  const lines: Array<{ startMs: number; text: string }> = []
  for (const origLine of (raw || '').split(/\r?\n/)) {
    const line = origLine.trimEnd()
    if (!line.trim()) continue

    RE_TIME_TAG.lastIndex = 0
    const tags: number[] = []
    let lastEnd = 0
    for (;;) {
      const m = RE_TIME_TAG.exec(line)
      if (!m) break
      lastEnd = RE_TIME_TAG.lastIndex
      const t = parseTimeTagMs(m[1], m[2], m[3])
      if (t != null) tags.push(t)
    }

    if (tags.length === 0) {
      meta.push(line)
      continue
    }

    const text = line.slice(lastEnd).trim()
    for (const t of tags) {
      lines.push({ startMs: t, text })
    }
  }
  lines.sort((a, b) => a.startMs - b.startMs)
  return { meta, lines }
}

export function parseLrc(original: string, translation?: string | null): Timeline {
  const orig = extractTimedLines(original || '')
  const trans = translation ? extractTimedLines(translation) : { meta: [], lines: [] as Array<{ startMs: number; text: string }> }

  // Build translation map by exact timestamp.
  const tmap = new Map<number, string>()
  for (const l of trans.lines) {
    if (!l.text.trim()) continue
    tmap.set(l.startMs, l.text)
  }

  const out: TimelineLine[] = []
  for (const l of orig.lines) {
    out.push({
      startMs: l.startMs,
      endMs: l.startMs, // filled later
      text: l.text,
      translationText: tmap.get(l.startMs) ?? null
    })
  }

  // Fill endMs using next line start.
  for (let i = 0; i < out.length; i++) {
    const cur = out[i]
    const next = out[i + 1]
    const nextStart = next ? next.startMs : null
    cur.endMs = nextStart != null && nextStart > cur.startMs ? nextStart : cur.startMs + 5000
  }

  return { meta: [...orig.meta, ...trans.meta], lines: out }
}

export function getActiveLine(tl: Timeline, positionMs: number): { index: number; progress01: number } {
  const lines = tl?.lines || []
  if (lines.length === 0) return { index: -1, progress01: 0 }

  const p = Math.max(0, Math.floor(positionMs || 0))

  // Binary search: last line with startMs <= p
  let lo = 0
  let hi = lines.length - 1
  let ans = -1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const v = lines[mid].startMs
    if (v <= p) {
      ans = mid
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  if (ans < 0) ans = 0
  const cur = lines[ans]
  const span = Math.max(1, cur.endMs - cur.startMs)
  const prog = (p - cur.startMs) / span
  return { index: ans, progress01: Math.max(0, Math.min(1, prog)) }
}

