import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

import { invoke } from '@tauri-apps/api/core'

import { lyricsSearch } from './lyricsApi'

describe('lyricsApi', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('lyricsSearch uses camelCase keys expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([])
    await lyricsSearch({
      title: 'Hello',
      artist: 'Adele',
      album: 'Hello',
      durationMs: 296000,
      limit: 7,
      strictMatch: true,
      servicesCsv: 'qq,netease,lrclib',
      timeoutMs: 8000
    })
    expect(invoke).toHaveBeenCalledWith('lyrics_search', {
      title: 'Hello',
      album: 'Hello',
      artist: 'Adele',
      durationMs: 296000,
      limit: 7,
      strictMatch: true,
      servicesCsv: 'qq,netease,lrclib',
      timeoutMs: 8000
    })
  })
})
