import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

import { invoke } from '@tauri-apps/api/core'

import { subtitleDownload, subtitleSearch } from './subtitleApi'

describe('subtitleApi', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('subtitleSearch uses camelCase keys expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([])
    await subtitleSearch({ query: 'a', minScore: 50, lang: 'zh', limit: 10 })
    expect(invoke).toHaveBeenCalledWith('subtitle_search', {
      query: 'a',
      minScore: 50,
      lang: 'zh',
      limit: 10
    })
  })

  it('subtitleDownload uses camelCase keys expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('/tmp/x.srt')
    const item: any = { id: 'x', name: 'n' }
    await subtitleDownload({ item, outDir: '/tmp', overwrite: false })
    expect(invoke).toHaveBeenCalledWith('subtitle_download', {
      item,
      outDir: '/tmp',
      overwrite: false
    })
  })
})

