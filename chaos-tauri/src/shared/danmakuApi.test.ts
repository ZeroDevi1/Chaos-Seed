import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

import { invoke } from '@tauri-apps/api/core'

import { fetchDanmakuImage } from './danmakuApi'

describe('danmakuApi', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('fetchDanmakuImage uses camelCase roomId key expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ mime: 'image/png', bytes: [1, 2, 3] })
    await fetchDanmakuImage({ url: 'https://example.com/a.png', site: 'bili', roomId: '1' })
    expect(invoke).toHaveBeenCalledWith('danmaku_fetch_image', {
      url: 'https://example.com/a.png',
      site: 'bili',
      roomId: '1'
    })
  })
})

