import type { AVPlayerOptions } from '@libmedia/avplayer'
// NOTE:
// - `@libmedia/avutil` exports some enums as ambient `const enum` in its typings, which conflicts with
//   our TS config (`verbatimModuleSyntax`).
// - Deep-importing the runtime enum from `@libmedia/avutil/dist/...` breaks Vite builds because the
//   package does not export that path.
// So we keep a small numeric subset here (values match libmedia's ESM enum output).
const AVCodecID = {
  AV_CODEC_ID_H264: 27,
  AV_CODEC_ID_HEVC: 173,
  AV_CODEC_ID_MPEG4: 12,
  AV_CODEC_ID_VVC: 196,
  AV_CODEC_ID_AV1: 225,
  AV_CODEC_ID_VP8: 139,
  AV_CODEC_ID_VP9: 167,
  AV_CODEC_ID_THEORA: 30,
  AV_CODEC_ID_MPEG2VIDEO: 2,
  AV_CODEC_ID_AAC: 86018,
  AV_CODEC_ID_MP3: 86017,
  AV_CODEC_ID_OPUS: 86076,
  AV_CODEC_ID_FLAC: 86028,
  AV_CODEC_ID_SPEEX: 86051,
  AV_CODEC_ID_VORBIS: 86021,
  AV_CODEC_ID_AC3: 86019,
  AV_CODEC_ID_EAC3: 86056,
  AV_CODEC_ID_DTS: 86020
} as const

import type { PlayerEngine, PlayerSource } from '../types'

// Same CDN base as 115Master (fastly jsdelivr). Keep remote WASM to avoid bundling complexity.
const CDN_URL_WASM = 'https://fastly.jsdelivr.net/gh/zhaohappy/libmedia@latest/dist'

type AVPlayerCtor = new (options: AVPlayerOptions) => {
  load: (url: string, opts?: any) => Promise<void>
  play: (opts?: any) => Promise<void>
  pause: () => Promise<void> | void
  setPlaybackRate: (rate: number) => void
  setVolume: (volume: number) => void
  resize: (width: number, height: number) => void
  destroy: () => Promise<void>
}

function getWasmUrl(type: any, codecId: any, _mediaType?: any): string {
  const DECODE_BASE_URL = `${CDN_URL_WASM}/decode`
  const RESAMPLE_BASE_URL = `${CDN_URL_WASM}/resample`
  const STRETCHPITCH_BASE_URL = `${CDN_URL_WASM}/stretchpitch`

  switch (type) {
    case 'decoder': {
      // PCM
      if (codecId && codecId >= 65536 && codecId <= 65572) {
        return `${DECODE_BASE_URL}/pcm-simd.wasm`
      }
      switch (codecId) {
        case AVCodecID.AV_CODEC_ID_H264:
          return `${DECODE_BASE_URL}/h264-simd.wasm`
        case AVCodecID.AV_CODEC_ID_HEVC:
          return `${DECODE_BASE_URL}/hevc-simd.wasm`
        case AVCodecID.AV_CODEC_ID_MPEG4:
          return `${DECODE_BASE_URL}/mpeg4-simd.wasm`
        case AVCodecID.AV_CODEC_ID_VVC:
          return `${DECODE_BASE_URL}/vvc-simd.wasm`
        case AVCodecID.AV_CODEC_ID_AV1:
          return `${DECODE_BASE_URL}/av1-simd.wasm`
        case AVCodecID.AV_CODEC_ID_VP8:
          return `${DECODE_BASE_URL}/vp8-simd.wasm`
        case AVCodecID.AV_CODEC_ID_VP9:
          return `${DECODE_BASE_URL}/vp9-simd.wasm`
        case AVCodecID.AV_CODEC_ID_THEORA:
          return `${DECODE_BASE_URL}/theora-simd.wasm`
        case AVCodecID.AV_CODEC_ID_MPEG2VIDEO:
          return `${DECODE_BASE_URL}/mpeg2video-simd.wasm`

        case AVCodecID.AV_CODEC_ID_AAC:
          return `${DECODE_BASE_URL}/aac-simd.wasm`
        case AVCodecID.AV_CODEC_ID_MP3:
          return `${DECODE_BASE_URL}/mp3-simd.wasm`
        case AVCodecID.AV_CODEC_ID_OPUS:
          return `${DECODE_BASE_URL}/opus-simd.wasm`
        case AVCodecID.AV_CODEC_ID_FLAC:
          return `${DECODE_BASE_URL}/flac-simd.wasm`
        case AVCodecID.AV_CODEC_ID_SPEEX:
          return `${DECODE_BASE_URL}/speex-simd.wasm`
        case AVCodecID.AV_CODEC_ID_VORBIS:
          return `${DECODE_BASE_URL}/vorbis-simd.wasm`
        case AVCodecID.AV_CODEC_ID_AC3:
          return `${DECODE_BASE_URL}/ac3-simd.wasm`
        case AVCodecID.AV_CODEC_ID_EAC3:
          return `${DECODE_BASE_URL}/eac3-simd.wasm`
        case AVCodecID.AV_CODEC_ID_DTS:
          return `${DECODE_BASE_URL}/dca-simd.wasm`
        default:
          // libmedia accepts empty string but it will fail later; surface a clear error.
          throw new Error(`Unsupported decoder codecId=${String(codecId)}`)
      }
    }
    case 'resampler':
      return `${RESAMPLE_BASE_URL}/resample-simd.wasm`
    case 'stretchpitcher':
      return `${STRETCHPITCH_BASE_URL}/stretchpitch-simd.wasm`
    default:
      throw new Error(`Unsupported wasm type=${String(type)}`)
  }
}

async function importAVPlayer(): Promise<AVPlayerCtor> {
  const mod: any = await import('@libmedia/avplayer')
  return (mod?.default ?? mod?.AVPlayer ?? mod) as AVPlayerCtor
}

export class AvPlayerEngine implements PlayerEngine {
  kind: PlayerEngine['kind'] = 'avplayer'

  private container: HTMLElement | null = null
  private player: any | null = null
  private ro: ResizeObserver | null = null
  private muted = false
  private volume01 = 1

  async init(container: HTMLElement): Promise<void> {
    this.container = container
    container.innerHTML = ''

    const AVPlayer = await importAVPlayer()
    const opts: AVPlayerOptions = {
      isLive: true,
      // Stability-first defaults for embedded webviews (WebView2):
      // - Hardware/WebCodecs paths can produce "audio-only / black video" on some drivers/builds.
      // We can later add a UI toggle to re-enable these for performance.
      enableHardware: false,
      enableWebCodecs: false,
      // WebGPU rendering can be flaky on some WebView2 builds; prefer stable Canvas/WebGL paths.
      enableWebGPU: false,
      enableWorker: true,
      preLoadTime: 60,
      container: container as HTMLDivElement,
      getWasm: (type: any, codecId: any, mediaType: any) => {
        try {
          return getWasmUrl(type, codecId, mediaType)
        } catch {
          return ''
        }
      }
    } as any

    this.player = new AVPlayer(opts as any)

    // Ensure the render surface (often a canvas) matches the container size; otherwise you can get
    // "audio only" with a 0x0 or tiny video surface.
    this.ro = new ResizeObserver(() => {
      const p = this.player
      const el = this.container
      if (!p || !el) return
      const w = Math.floor(el.clientWidth || 0)
      const h = Math.floor(el.clientHeight || 0)
      if (w <= 0 || h <= 0) return
      try {
        p.resize(w, h)
      } catch {
        // ignore
      }
    })
    try {
      this.ro.observe(container)
    } catch {
      // ignore
    }
  }

  async load(source: PlayerSource): Promise<void> {
    if (!this.player) throw new Error('avplayer engine not initialized')
    const url = (source.url || '').toString().trim()
    if (!url) throw new Error('empty url')
    const headers: Record<string, string> = {}
    const referer = (source.referer ?? '').toString().trim()
    if (referer) headers.Referer = referer
    const ua = (source.user_agent ?? '').toString().trim()
    if (ua) headers['User-Agent'] = ua
    await this.player.load(url, {
      isLive: true,
      http: {
        headers
      }
    })
    // Force one resize after load too (some builds only create the canvas on load()).
    try {
      const el = this.container
      if (el) this.player.resize(Math.floor(el.clientWidth || 0), Math.floor(el.clientHeight || 0))
    } catch {
      // ignore
    }
    // Re-apply audio state after load.
    this.setVolume(this.volume01)
    this.setMuted(this.muted)
  }

  async play(): Promise<void> {
    if (!this.player) return
    await this.player.play({ subtitle: false })
  }

  async pause(): Promise<void> {
    if (!this.player) return
    await this.player.pause()
  }

  setMuted(muted: boolean): void {
    this.muted = !!muted
    this.setVolume(this.volume01)
  }

  setVolume(volume01: number): void {
    const v = Number.isFinite(volume01) ? Math.max(0, Math.min(1, volume01)) : 1
    this.volume01 = v
    if (!this.player) return
    // libmedia's volume is not strictly documented; 115Master scales (0..100)*3.
    const out = this.muted ? 0 : this.volume01 * 3
    try {
      this.player.setVolume(out)
    } catch {
      // ignore
    }
  }

  async destroy(): Promise<void> {
    const ro = this.ro
    this.ro = null
    if (ro) {
      try {
        ro.disconnect()
      } catch {
        // ignore
      }
    }
    const p = this.player
    this.player = null
    if (p) {
      try {
        await p.pause?.()
      } catch {
        // ignore
      }
      try {
        await p.destroy?.()
      } catch {
        // ignore
      }
    }
    if (this.container) this.container.innerHTML = ''
    this.container = null
  }
}
