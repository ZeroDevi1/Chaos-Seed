import { describe, expect, it } from 'vitest'

import { getActiveLine, parseLrc } from './lyricsSync'

describe('lyricsSync', () => {
  it('handles empty input', () => {
    const tl = parseLrc('', null)
    expect(tl.lines.length).toBe(0)
    expect(getActiveLine(tl, 0)).toEqual({ index: -1, progress01: 0 })
  })

  it('parses timed lrc and infers endMs', () => {
    const tl = parseLrc('[00:01.00]a\n[00:02.50]b\n', null)
    expect(tl.lines.length).toBe(2)
    expect(tl.lines[0].startMs).toBe(1000)
    expect(tl.lines[0].endMs).toBe(2500)
    expect(tl.lines[1].startMs).toBe(2500)
    expect(tl.lines[1].endMs).toBe(2500 + 5000)
  })

  it('merges translation by exact timestamp', () => {
    const tl = parseLrc('[00:01.00]a\n[00:02.00]b\n', '[00:01.00]ta\n')
    expect(tl.lines[0].translationText).toBe('ta')
    expect(tl.lines[1].translationText).toBe(null)
  })

  it('getActiveLine returns stable index and progress', () => {
    const tl = parseLrc('[00:01.00]a\n[00:03.00]b\n', null)
    expect(getActiveLine(tl, 0).index).toBe(0)
    expect(getActiveLine(tl, 1000).index).toBe(0)
    const x = getActiveLine(tl, 2000)
    expect(x.index).toBe(0)
    expect(x.progress01).toBeGreaterThan(0)
    expect(getActiveLine(tl, 3000).index).toBe(1)
  })
})

