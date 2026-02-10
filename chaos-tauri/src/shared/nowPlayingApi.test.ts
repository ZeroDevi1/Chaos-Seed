import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

import { invoke } from '@tauri-apps/api/core'

import { nowPlayingSnapshot } from './nowPlayingApi'

describe('nowPlayingApi', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('nowPlayingSnapshot uses camelCase keys expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ supported: false, now_playing: null, sessions: [], retrieved_at_unix_ms: 0, picked_app_id: null })
    await nowPlayingSnapshot({ includeThumbnail: true, maxThumbnailBytes: 123, maxSessions: 7 })
    expect(invoke).toHaveBeenCalledWith('now_playing_snapshot', {
      includeThumbnail: true,
      maxThumbnailBytes: 123,
      maxSessions: 7
    })
  })
})
