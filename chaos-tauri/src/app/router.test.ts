import { describe, expect, it } from 'vitest'

import { createAppRouter } from './router'

describe('router', () => {
  it('resolves known routes', () => {
    const router = createAppRouter()
    expect(router.resolve('/').matched).toHaveLength(1)
    expect(router.resolve('/subtitle').matched).toHaveLength(1)
    expect(router.resolve('/danmaku').matched).toHaveLength(1)
    expect(router.resolve('/settings').matched).toHaveLength(1)
    expect(router.resolve('/about').matched).toHaveLength(1)
  })
})

