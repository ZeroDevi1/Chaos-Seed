import { describe, expect, it } from 'vitest'

import { choosePauseStrategy } from './playbackPolicy'

describe('playbackPolicy', () => {
  it('uses stop strategy for avplayer live streams (pause is unreliable for live flv)', () => {
    expect(choosePauseStrategy({ engineKind: 'avplayer', isLive: true })).toBe('stop')
  })

  it('uses pause strategy for non-avplayer engines', () => {
    expect(choosePauseStrategy({ engineKind: 'native', isLive: true })).toBe('pause')
    expect(choosePauseStrategy({ engineKind: 'hls', isLive: true })).toBe('pause')
  })

  it('uses pause strategy for avplayer VOD', () => {
    expect(choosePauseStrategy({ engineKind: 'avplayer', isLive: false })).toBe('pause')
  })
})

