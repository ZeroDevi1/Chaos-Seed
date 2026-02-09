import type { PlayerEngine, PlayerSource } from '../types'

function once<T extends Event>(target: EventTarget, name: string, timeoutMs: number): Promise<T> {
  return new Promise((resolve, reject) => {
    let done = false
    const onOk = (ev: Event) => {
      if (done) return
      done = true
      cleanup()
      resolve(ev as T)
    }
    const onErr = () => {
      if (done) return
      done = true
      cleanup()
      reject(new Error(`media error: ${name}`))
    }
    const t = window.setTimeout(() => {
      if (done) return
      done = true
      cleanup()
      reject(new Error(`timeout waiting for ${name}`))
    }, Math.max(1, timeoutMs))

    const cleanup = () => {
      window.clearTimeout(t)
      target.removeEventListener(name, onOk as EventListener, { capture: true } as any)
      target.removeEventListener('error', onErr as EventListener, { capture: true } as any)
    }

    target.addEventListener(name, onOk as EventListener, { capture: true, once: true } as any)
    target.addEventListener('error', onErr as EventListener, { capture: true, once: true } as any)
  })
}

export class NativeEngine implements PlayerEngine {
  kind: PlayerEngine['kind'] = 'native'

  private container: HTMLElement | null = null
  private video: HTMLVideoElement | null = null
  private muted = false
  private volume01 = 1

  async init(container: HTMLElement): Promise<void> {
    this.container = container
    container.innerHTML = ''

    const video = document.createElement('video')
    video.playsInline = true
    video.autoplay = false
    video.controls = false
    video.muted = this.muted
    video.volume = this.volume01
    video.style.width = '100%'
    video.style.height = '100%'
    video.style.objectFit = 'contain'
    video.style.background = '#000'

    container.appendChild(video)
    this.video = video
  }

  async load(source: PlayerSource): Promise<void> {
    if (!this.video) throw new Error('native engine not initialized')
    const url = (source.url || '').toString().trim()
    if (!url) throw new Error('empty url')

    // Clear previous network activity.
    try {
      this.video.pause()
    } catch {
      // ignore
    }
    this.video.removeAttribute('src')
    this.video.load()

    this.video.src = url
    this.video.load()

    // Live sources may not fire the full metadata path reliably; "canplay" is a good compromise.
    await once(this.video, 'canplay', 15_000)
  }

  async play(): Promise<void> {
    if (!this.video) return
    await this.video.play()
  }

  async pause(): Promise<void> {
    if (!this.video) return
    this.video.pause()
  }

  setMuted(muted: boolean): void {
    this.muted = !!muted
    if (this.video) this.video.muted = this.muted
  }

  setVolume(volume01: number): void {
    const v = Number.isFinite(volume01) ? Math.max(0, Math.min(1, volume01)) : 1
    this.volume01 = v
    if (this.video) this.video.volume = this.volume01
  }

  async destroy(): Promise<void> {
    const video = this.video
    this.video = null
    if (video) {
      try {
        video.pause()
      } catch {
        // ignore
      }
      // Clearing src + load is the most reliable way to terminate the network request in webviews.
      try {
        video.removeAttribute('src')
        video.load()
      } catch {
        // ignore
      }
      try {
        video.remove()
      } catch {
        // ignore
      }
    }
    if (this.container) {
      this.container.innerHTML = ''
    }
    this.container = null
  }
}

