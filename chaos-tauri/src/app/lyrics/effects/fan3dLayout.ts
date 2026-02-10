import type { LayoutEffect } from './types'

export class Fan3DLayoutEffect implements LayoutEffect {
  apply(lines: HTMLElement[], activeIndex: number) {
    const n = lines.length
    for (let i = 0; i < n; i++) {
      const el = lines[i]
      const off = i - activeIndex
      const abs = Math.abs(off)
      const rot = Math.max(-38, Math.min(38, off * 8))
      const z = Math.max(-120, 40 - abs * 26)
      const y = off * 18
      const s = Math.max(0.84, 1.0 - abs * 0.04)
      const op = Math.max(0.20, 1.0 - abs * 0.18)
      el.style.transform = `translate3d(0px, ${y}px, ${z}px) rotateY(${rot}deg) scale(${s})`
      el.style.opacity = String(op)
    }
  }

  dispose() {
    // no-op
  }
}

