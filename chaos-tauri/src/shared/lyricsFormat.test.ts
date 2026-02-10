import { describe, expect, it } from 'vitest'

import { alignTranslation, parseLrc } from './lyricsFormat'

describe('lyricsFormat', () => {
  it('parseLrc parses meta and timed lines', () => {
    const rows = parseLrc('[ti:Hello]\n[00:01.00]a\n[00:02.50]b')
    expect(rows.length).toBe(3)
    expect(rows[0].kind).toBe('meta')
    expect(rows[1].kind).toBe('line')
    expect(rows[1].timeMs).toBe(1000)
    expect(rows[1].text).toBe('a')
    expect(rows[2].timeMs).toBe(2500)
  })

  it('alignTranslation aligns by timestamp when possible', () => {
    const orig = '[00:01.00]a\n[00:02.00]b'
    const trans = '[00:01.00]A\n[00:02.00]B'
    const out = alignTranslation(orig, trans)
    expect(out).toEqual([
      { original: 'a', translation: 'A', isMeta: false },
      { original: 'b', translation: 'B', isMeta: false }
    ])
  })

  it('alignTranslation falls back to index alignment when translation has no timestamps', () => {
    const orig = '[00:01.00]a\n[00:02.00]b'
    const trans = 'A\nB'
    const out = alignTranslation(orig, trans)
    expect(out).toEqual([
      { original: 'a', translation: 'A', isMeta: false },
      { original: 'b', translation: 'B', isMeta: false }
    ])
  })
})

