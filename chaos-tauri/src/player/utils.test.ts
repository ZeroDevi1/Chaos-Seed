import { describe, expect, it } from 'vitest'

import { inferStreamType } from './utils'

describe('inferStreamType', () => {
  it('detects m3u8', () => {
    expect(inferStreamType('https://a/b.m3u8')).toBe('hls')
    expect(inferStreamType('https://a/b.m3u8?x=1')).toBe('hls')
  })

  it('detects flv', () => {
    expect(inferStreamType('https://a/b.flv')).toBe('flv')
    expect(inferStreamType('https://a/b.flv?token=1')).toBe('flv')
  })

  it('falls back to native', () => {
    expect(inferStreamType('https://a/b.mp4')).toBe('native')
    expect(inferStreamType('')).toBe('native')
  })
})

