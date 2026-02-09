import Hls from 'hls.js'
import type { HlsConfig } from 'hls.js'

import type { PlayerEngine, PlayerSource } from '../types'

// Keep this aligned with 115Master's defaults, but trimmed for our use-case.
const DEFAULT_CONFIG: Partial<HlsConfig> = {
  autoStartLoad: true,
  maxBufferLength: 1200,
  lowLatencyMode: true,
  startPosition: -1,
  debug: false
}

export class HlsEngine implements PlayerEngine {
  kind: PlayerEngine['kind'] = 'hls'

  private container: HTMLElement | null = null
  private video: HTMLVideoElement | null = null
  private hls: Hls | null = null
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

    this.hls = new Hls({ ...(DEFAULT_CONFIG as any) })
  }

  async load(source: PlayerSource): Promise<void> {
    const url = (source.url || '').toString().trim()
    if (!url) throw new Error('empty url')
    if (!this.video || !this.hls) throw new Error('hls engine not initialized')

    // Terminate any previous stream.
    try {
      this.video.pause()
    } catch {
      // ignore
    }
    this.hls.stopLoad()
    this.hls.detachMedia()

    await new Promise<void>((resolve, reject) => {
      if (!this.hls || !this.video) return reject(new Error('hls engine not initialized'))
      const h = this.hls
      const v = this.video
      const t = window.setTimeout(() => {
        cleanup()
        reject(new Error('hls manifest timeout'))
      }, 15_000)
      const cleanup = () => {
        window.clearTimeout(t)
        h.off(Hls.Events.ERROR, onErr)
        h.off(Hls.Events.MANIFEST_PARSED, onOk)
      }
      const onOk = () => {
        cleanup()
        resolve()
      }
      const onErr = (_: unknown, data: any) => {
        if (data?.fatal) {
          cleanup()
          reject(new Error(data?.details || data?.type || 'hls fatal error'))
        }
      }
      h.on(Hls.Events.ERROR, onErr)
      h.on(Hls.Events.MANIFEST_PARSED, onOk)
      h.loadSource(url)
      h.attachMedia(v)
    })
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
    const h = this.hls
    this.hls = null
    if (h) {
      try {
        h.destroy()
      } catch {
        // ignore
      }
    }

    const video = this.video
    this.video = null
    if (video) {
      try {
        video.pause()
      } catch {
        // ignore
      }
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

    if (this.container) this.container.innerHTML = ''
    this.container = null
  }
}
