import { describe, expect, it } from 'vitest'

import { installDisableZoom } from './disableZoom'

describe('installDisableZoom', () => {
  it('prevents default for Ctrl/Cmd + + / - / 0', () => {
    const cleanup = installDisableZoom()
    try {
      const evPlus = new KeyboardEvent('keydown', { key: '=', ctrlKey: true, cancelable: true })
      window.dispatchEvent(evPlus)
      expect(evPlus.defaultPrevented).toBe(true)

      const evMinus = new KeyboardEvent('keydown', { key: '-', ctrlKey: true, cancelable: true })
      window.dispatchEvent(evMinus)
      expect(evMinus.defaultPrevented).toBe(true)

      const evZero = new KeyboardEvent('keydown', { key: '0', ctrlKey: true, cancelable: true })
      window.dispatchEvent(evZero)
      expect(evZero.defaultPrevented).toBe(true)
    } finally {
      cleanup()
    }
  })

  it('prevents default for Ctrl+Wheel zoom', () => {
    const cleanup = installDisableZoom()
    try {
      const ev = new WheelEvent('wheel', { ctrlKey: true, cancelable: true })
      window.dispatchEvent(ev)
      expect(ev.defaultPrevented).toBe(true)
    } finally {
      cleanup()
    }
  })

  it('removes listeners on cleanup', () => {
    const cleanup = installDisableZoom()
    cleanup()

    const ev = new KeyboardEvent('keydown', { key: '0', ctrlKey: true, cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(false)
  })
})

