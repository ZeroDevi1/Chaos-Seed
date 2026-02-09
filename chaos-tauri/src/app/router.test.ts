import { describe, expect, it } from 'vitest'

import { getHashPath, resolveRoute } from './router'

describe('router', () => {
  it('resolves known routes', () => {
    expect(resolveRoute('/')?.path).toBe('/')
    expect(resolveRoute('/subtitle')?.path).toBe('/subtitle')
    expect(resolveRoute('/danmaku')?.path).toBe('/danmaku')
    expect(resolveRoute('/danmaku')?.keepAlive).toBe(true)
    expect(resolveRoute('/settings')?.path).toBe('/settings')
    expect(resolveRoute('/about')?.path).toBe('/about')
  })

  it('returns null for unknown routes', () => {
    expect(resolveRoute('/nope')).toBeNull()
    expect(resolveRoute('subtitle')).toBeNull()
  })

  it('parses hash into a normalized path', () => {
    expect(getHashPath('')).toBe('/')
    expect(getHashPath('#/')).toBe('/')
    expect(getHashPath('#/subtitle')).toBe('/subtitle')
    expect(getHashPath('#subtitle')).toBe('/subtitle')
    expect(getHashPath('#/subtitle/')).toBe('/subtitle')
  })
})
