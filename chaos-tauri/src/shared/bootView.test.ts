import { describe, expect, it } from 'vitest'

import { resolveView } from './bootView'

describe('resolveView', () => {
  it('prefers boot.view over query params', () => {
    expect(resolveView({ boot: { view: 'chat' }, search: '?view=overlay', label: 'overlay' })).toBe('chat')
    expect(resolveView({ boot: { view: 'overlay' }, search: '?view=chat', label: 'chat' })).toBe('overlay')
    expect(resolveView({ boot: { view: 'player' }, search: '?view=chat', label: 'main' })).toBe('player')
  })

  it('falls back to query params when boot is missing/invalid', () => {
    expect(resolveView({ boot: undefined, search: '?view=chat', label: 'overlay' })).toBe('chat')
    expect(resolveView({ boot: null, search: '?view=overlay', label: 'chat' })).toBe('overlay')
    expect(resolveView({ boot: { view: 'nope' }, search: '?view=chat', label: 'overlay' })).toBe('chat')
    expect(resolveView({ boot: undefined, search: '?view=player', label: 'main' })).toBe('player')
  })

  it('falls back to window label when boot and query are missing/invalid', () => {
    expect(resolveView({ boot: undefined, search: '', label: 'chat' })).toBe('chat')
    expect(resolveView({ boot: undefined, search: '', label: 'overlay' })).toBe('overlay')
    expect(resolveView({ boot: undefined, search: '', label: 'MAIN' })).toBe('main')
    expect(resolveView({ boot: undefined, search: '', label: 'player' })).toBe('player')
  })

  it('defaults to main when boot, query, and label do not indicate a supported view', () => {
    expect(resolveView({ boot: undefined, search: '', label: undefined })).toBe('main')
    expect(resolveView({ boot: {}, search: '?x=1', label: 'nope' })).toBe('main')
    expect(resolveView({ boot: { view: 123 }, search: '?view=nope', label: '???' })).toBe('main')
  })
})
