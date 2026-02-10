import { beforeEach, describe, expect, it, vi } from 'vitest'

// Mock tauri invoke so we can assert the payload keys (camelCase vs snake_case).
vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn()
  }
})

import { invoke } from '@tauri-apps/api/core'

import { resolveVariant } from './livestreamApi'

describe('livestreamApi.resolveVariant', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('uses camelCase keys expected by tauri commands', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: 'x', label: 'l', quality: 1, url: 'u', backup_urls: [] })
    await resolveVariant('douyu', '999', 'douyu:1:高清')
    expect(invoke).toHaveBeenCalledWith('livestream_resolve_variant', {
      site: 'douyu',
      roomId: '999',
      variantId: 'douyu:1:高清'
    })
  })
})

