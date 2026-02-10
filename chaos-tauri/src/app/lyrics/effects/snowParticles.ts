import type { ParticleEffect } from './types'

type Flake = { x: number; y: number; r: number; vy: number; vx: number; a: number }

export class SnowParticlesEffect implements ParticleEffect {
  private canvas: HTMLCanvasElement | null = null
  private ctx: CanvasRenderingContext2D | null = null
  private active = false
  private raf = 0
  private w = 1
  private h = 1
  private flakes: Flake[] = []

  mount(canvas: HTMLCanvasElement) {
    this.canvas = canvas
    this.ctx = canvas.getContext('2d')
    this.resize(canvas.width, canvas.height)
  }

  resize(width: number, height: number) {
    this.w = Math.max(1, Math.floor(width))
    this.h = Math.max(1, Math.floor(height))
    const want = Math.min(220, Math.max(30, Math.floor((this.w * this.h) / 25_000)))
    if (this.flakes.length > want) this.flakes.length = want
    while (this.flakes.length < want) this.flakes.push(this.makeFlake(true))
  }

  setActive(active: boolean) {
    this.active = active
    if (!this.active) {
      cancelAnimationFrame(this.raf)
      return
    }
    const loop = () => {
      this.raf = requestAnimationFrame(loop)
      this.step()
      this.draw()
    }
    cancelAnimationFrame(this.raf)
    this.raf = requestAnimationFrame(loop)
  }

  private makeFlake(randomY: boolean): Flake {
    const r = 0.6 + Math.random() * 2.2
    return {
      x: Math.random() * this.w,
      y: randomY ? Math.random() * this.h : -10 - Math.random() * 30,
      r,
      vy: 16 + Math.random() * 52,
      vx: -10 + Math.random() * 20,
      a: 0.12 + Math.random() * 0.38
    }
  }

  private step() {
    const dt = 1 / 60
    for (let i = 0; i < this.flakes.length; i++) {
      const f = this.flakes[i]
      f.x += f.vx * dt
      f.y += f.vy * dt
      if (f.y > this.h + 10) this.flakes[i] = this.makeFlake(false)
      if (f.x < -10) f.x = this.w + 10
      if (f.x > this.w + 10) f.x = -10
    }
  }

  private draw() {
    const ctx = this.ctx
    const c = this.canvas
    if (!ctx || !c) return
    ctx.clearRect(0, 0, this.w, this.h)
    ctx.save()
    ctx.fillStyle = 'rgba(255,255,255,0.9)'
    for (const f of this.flakes) {
      ctx.globalAlpha = f.a
      ctx.beginPath()
      ctx.arc(f.x, f.y, f.r, 0, Math.PI * 2)
      ctx.fill()
    }
    ctx.restore()
  }

  dispose() {
    cancelAnimationFrame(this.raf)
    this.raf = 0
    this.canvas = null
    this.ctx = null
    this.flakes = []
  }
}

