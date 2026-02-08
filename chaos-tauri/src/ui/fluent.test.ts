import { beforeEach, describe, expect, it, vi } from 'vitest'

const baseSet = vi.fn()
const accentSet = vi.fn()

vi.mock('@fluentui/web-components', () => {
  return {
    // Tokens
    baseLayerLuminance: { setValueFor: baseSet },
    accentBaseColor: { setValueFor: accentSet },
    StandardLuminance: { DarkMode: 0.23, LightMode: 1 },
    SwatchRGB: { create: vi.fn(() => ({ __swatch: true })) },

    // Design system + component registration (no-op in tests)
    provideFluentDesignSystem: vi.fn(() => ({ register: vi.fn() })),
    fluentButton: vi.fn(() => ({})),
    fluentCard: vi.fn(() => ({})),
    fluentTextField: vi.fn(() => ({})),
    fluentNumberField: vi.fn(() => ({})),
    fluentSelect: vi.fn(() => ({})),
    fluentOption: vi.fn(() => ({}))
  }
})

describe('applyFluentTokens', () => {
  beforeEach(() => {
    baseSet.mockClear()
    accentSet.mockClear()
    document.body.innerHTML = '<div id="app"></div>'
  })

  it('applies tokens to document.body and avoids redundant writes', async () => {
    vi.resetModules()
    const { applyFluentTokens } = await import('./fluent')

    const host = document.body
    expect(host).toBeInstanceOf(HTMLElement)

    applyFluentTokens('light')
    expect(baseSet).toHaveBeenCalledTimes(1)
    expect(baseSet).toHaveBeenLastCalledWith(host, 1)
    expect(accentSet).toHaveBeenCalledTimes(1)
    expect(accentSet).toHaveBeenLastCalledWith(host, expect.anything())

    // Same theme again should not write tokens again.
    applyFluentTokens('light')
    expect(baseSet).toHaveBeenCalledTimes(1)
    expect(accentSet).toHaveBeenCalledTimes(1)

    // Theme change should update luminance but not re-apply accent (it doesn't change).
    applyFluentTokens('dark')
    expect(baseSet).toHaveBeenCalledTimes(2)
    expect(baseSet).toHaveBeenLastCalledWith(host, 0.23)
    expect(accentSet).toHaveBeenCalledTimes(1)
  })
})
