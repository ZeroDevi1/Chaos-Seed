import { describe, expect, it } from 'vitest'

import { calcHeroFromRect } from './liveSourceHeroRect'

describe('calcHeroFromRect', () => {
  it('converts dom rect to physical rect using scaleFactor', () => {
    const r = calcHeroFromRect({ x: 100, y: 50 }, { left: 10, top: 20, width: 200, height: 100 }, 2)
    expect(r).toEqual({ x: 120, y: 90, width: 400, height: 200 })
  })

  it('clamps invalid scaleFactor to 1', () => {
    const r = calcHeroFromRect({ x: 0, y: 0 }, { left: 1.4, top: 2.6, width: 0, height: -3 }, 0)
    expect(r.width).toBe(1)
    expect(r.height).toBe(1)
  })
})
