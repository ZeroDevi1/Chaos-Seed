import { describe, expect, it } from 'vitest'

import type { LiveManifest, StreamVariant } from './livestreamTypes'
import { mergeVariant, pickDefaultVariant } from './livestreamUtils'

function man(variants: StreamVariant[]): LiveManifest {
  return {
    site: 'bili_live',
    room_id: '1',
    raw_input: 'https://live.bilibili.com/1',
    info: { title: 't', is_living: true },
    playback: { referer: 'https://live.bilibili.com/' },
    variants
  }
}

describe('pickDefaultVariant', () => {
  it('prefers the highest-quality variant that has a url', () => {
    const m = man([
      { id: 'a', label: 'HD', quality: 1000, url: 'https://x/1000.flv', backup_urls: [] },
      { id: 'b', label: 'Original', quality: 2000, url: null, backup_urls: [] },
      { id: 'c', label: 'SD', quality: 500, url: 'https://x/500.flv', backup_urls: [] }
    ])
    expect(pickDefaultVariant(m)?.id).toBe('a')
  })

  it('falls back to the first variant if none has a url', () => {
    const m = man([
      { id: 'a', label: 'HD', quality: 1000, url: null, backup_urls: [] },
      { id: 'b', label: 'Original', quality: 2000, url: null, backup_urls: [] }
    ])
    expect(pickDefaultVariant(m)?.id).toBe('a')
  })
})

describe('mergeVariant', () => {
  it('updates a matching variant by id', () => {
    const m = man([
      { id: 'a', label: 'HD', quality: 1000, url: null, backup_urls: [] },
      { id: 'b', label: 'Original', quality: 2000, url: 'https://old', backup_urls: ['https://old2'] }
    ])
    const next = mergeVariant(m, {
      id: 'b',
      label: 'Original',
      quality: 2000,
      url: 'https://new',
      backup_urls: ['https://bak']
    })
    expect(next).not.toBe(m)
    expect(next.variants.find((v) => v.id === 'b')?.url).toBe('https://new')
    expect(next.variants.find((v) => v.id === 'b')?.backup_urls).toEqual(['https://bak'])
  })

  it('returns the original manifest if the variant is not found', () => {
    const m = man([{ id: 'a', label: 'HD', quality: 1000, url: null, backup_urls: [] }])
    expect(mergeVariant(m, { id: 'nope', label: 'x', quality: 1, url: 'u', backup_urls: [] })).toBe(m)
  })
})

