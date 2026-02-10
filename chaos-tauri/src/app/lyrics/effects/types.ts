export type EffectActive = {
  active: boolean
}

export interface BackgroundEffect {
  mount(canvas: HTMLCanvasElement): void
  resize(width: number, height: number): void
  setActive(active: boolean): void
  dispose(): void
}

export interface ParticleEffect {
  mount(canvas: HTMLCanvasElement): void
  resize(width: number, height: number): void
  setActive(active: boolean): void
  dispose(): void
}

export interface LayoutEffect {
  apply(lines: HTMLElement[], activeIndex: number): void
  dispose(): void
}

