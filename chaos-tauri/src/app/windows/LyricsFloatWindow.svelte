<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import type { LyricsSearchResult, NowPlayingSession } from '@/shared/types'
  import { getActiveLine, parseLrc, type Timeline } from '@/shared/lyricsSync'
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'
  import { FluidBackgroundEffect } from '@/app/lyrics/effects/fluidBackground'
  import { SnowParticlesEffect } from '@/app/lyrics/effects/snowParticles'
  import type { BackgroundEffect, ParticleEffect } from '@/app/lyrics/effects/types'

  type NowPlayingStatePayload = {
    supported: boolean
    app_id: string | null
    playback_status: string | null
    title: string | null
    artist: string | null
    album_title: string | null
    position_ms: number | null
    duration_ms: number | null
    retrieved_at_unix_ms: number
    genres?: string[] | null
    song_id?: string | null
  }

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null
  let rootEl: HTMLDivElement | null = null

  let item: LyricsSearchResult | null = null
  let timeline: Timeline | null = null
  let activeIndex = -1

  let nowPlaying: NowPlayingSession | null = null
  let nowRetrievedAtMs = 0

  let clickThrough = false
  let fading = false

  let bgCanvas: HTMLCanvasElement | null = null
  let snowCanvas: HTMLCanvasElement | null = null
  let bgEffect: BackgroundEffect | null = null
  let particleEffect: ParticleEffect | null = null
  let lastNowEventAt = 0
  let lastLyricsEventAt = 0

  function applyLyrics(next: LyricsSearchResult | null) {
    item = next
    timeline = item ? parseLrc(item.lyrics_original, item.lyrics_translation ?? null) : null
    activeIndex = -1
    lastLyricsEventAt = Date.now()
  }

  function applyNowPlaying(p: NowPlayingStatePayload) {
    if (!p?.supported) {
      nowPlaying = null
      nowRetrievedAtMs = 0
      bgEffect?.setActive(false)
      particleEffect?.setActive(false)
      lastNowEventAt = Date.now()
      return
    }
    nowPlaying = {
      app_id: p.app_id || '',
      is_current: true,
      playback_status: p.playback_status || 'Unknown',
      title: p.title,
      artist: p.artist,
      album_title: p.album_title,
      position_ms: p.position_ms,
      duration_ms: p.duration_ms,
      genres: p.genres ?? [],
      song_id: p.song_id ?? null,
      thumbnail: null,
      error: null
    }
    nowRetrievedAtMs = (p.retrieved_at_unix_ms || 0) as number
    fading = (nowPlaying.playback_status || '').toLowerCase() !== 'playing'
    const playing = (nowPlaying.playback_status || '').toLowerCase() === 'playing'
    bgEffect?.setActive(playing)
    particleEffect?.setActive(playing)
    lastNowEventAt = Date.now()
  }

  async function startDrag() {
    if (!win) return
    try {
      await win.startDragging()
    } catch {
      // ignore
    }
  }

  function effectivePositionMs(): number {
    if (!nowPlaying) return 0
    const pos = typeof nowPlaying.position_ms === 'number' ? nowPlaying.position_ms : 0
    const playing = (nowPlaying.playback_status || '').toLowerCase() === 'playing'
    if (!playing) return pos
    const dt = Date.now() - nowRetrievedAtMs
    return pos + Math.max(0, dt)
  }

  function currentLines(): Array<{ text: string; trans: string | null; strong: boolean }> {
    const tl = timeline?.lines || []
    if (tl.length === 0) return []
    const idx = activeIndex >= 0 ? activeIndex : 0
    const cur = tl[idx]
    const next = tl[idx + 1]
    const out: Array<{ text: string; trans: string | null; strong: boolean }> = []
    out.push({ text: cur.text, trans: cur.translationText ?? null, strong: true })
    if (next) out.push({ text: next.text, trans: next.translationText ?? null, strong: false })
    return out
  }

  async function applyClickThrough(next: boolean) {
    if (!win) {
      clickThrough = false
      return
    }
    try {
      await win.setIgnoreCursorEvents(next)
      clickThrough = next
    } catch {
      clickThrough = false
    }
  }

  onMount(() => {
    let disposed = false
    let unLyrics: (() => void) | undefined
    let unNow: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopResize: (() => void) | undefined
    let stopPoll: (() => void) | undefined

    // Transparent window: webview background must be transparent as well.
    document.documentElement.style.background = 'transparent'
    document.body.style.background = 'transparent'

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    const cleanup = () => {
      disposed = true
      unLyrics?.()
      unNow?.()
      stopKey?.()
      stopAnim?.()
      stopResize?.()
      stopPoll?.()
      bgEffect?.dispose()
      particleEffect?.dispose()
    }
    window.addEventListener('beforeunload', cleanup, { capture: true, once: true })

    // Default: interactive; user can toggle click-through with F2.
    void applyClickThrough(false)

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') {
        void win?.close()
        return
      }
      if (ev.key === 'F2') {
        ev.preventDefault()
        void applyClickThrough(!clickThrough)
      }
    }
    window.addEventListener('keydown', onKey, true)
    stopKey = () => window.removeEventListener('keydown', onKey, true)

    const applyCanvasSize = () => {
      if (!rootEl) return
      const dpr = Math.max(1, window.devicePixelRatio || 1)
      const w = Math.max(1, Math.floor(rootEl.clientWidth * dpr))
      const h = Math.max(1, Math.floor(rootEl.clientHeight * dpr))
      if (bgCanvas) {
        bgCanvas.width = w
        bgCanvas.height = h
        bgEffect?.resize(w, h)
      }
      if (snowCanvas) {
        snowCanvas.width = w
        snowCanvas.height = h
        particleEffect?.resize(w, h)
      }
    }
    applyCanvasSize()
    const ro = new ResizeObserver(() => applyCanvasSize())
    if (rootEl) ro.observe(rootEl)
    stopResize = () => ro.disconnect()

    void (async () => {
      try {
        const s = (await invoke('lyrics_settings_get')) as any
        const bg = s?.effects?.background_effect || 'none'
        const pt = s?.effects?.particle_effect || 'none'
        const playing = (nowPlaying?.playback_status || '').toLowerCase() === 'playing'

        if (bg === 'fluid' && bgCanvas) {
          bgEffect = new FluidBackgroundEffect()
          bgEffect.mount(bgCanvas)
          bgEffect.setActive(playing)
        }
        if (pt === 'snow' && snowCanvas) {
          particleEffect = new SnowParticlesEffect()
          particleEffect.mount(snowCanvas)
          particleEffect.setActive(playing)
        }
        applyCanvasSize()
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const cur = (await invoke('lyrics_get_current')) as LyricsSearchResult | null
        if (!disposed) applyLyrics(cur)
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const u = await listen<LyricsSearchResult | null>('lyrics_current_changed', (e) => {
          applyLyrics((e as unknown as { payload: LyricsSearchResult | null }).payload)
        })
        if (disposed) return u()
        unLyrics = u
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const u = await listen<NowPlayingStatePayload>('now_playing_state_changed', (e) => {
          applyNowPlaying(e.payload)
        })
        if (disposed) return u()
        unNow = u
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const s = await nowPlayingSnapshot({ includeThumbnail: false, maxThumbnailBytes: 0, maxSessions: 1 })
        const np = (s as any)?.now_playing
        if (disposed || !np) return
        applyNowPlaying({
          supported: !!(s as any)?.supported,
          app_id: np.app_id ?? null,
          playback_status: np.playback_status ?? null,
          title: np.title ?? null,
          artist: np.artist ?? null,
          album_title: np.album_title ?? null,
          position_ms: np.position_ms ?? null,
          duration_ms: np.duration_ms ?? null,
          retrieved_at_unix_ms: typeof (s as any)?.retrieved_at_unix_ms === 'number' ? (s as any).retrieved_at_unix_ms : Date.now(),
          genres: np.genres ?? [],
          song_id: np.song_id ?? null
        })
      } catch {
        // ignore
      }
    })()

    // Robustness: in case event listeners fail (or miss events), poll lightly.
    const poll = window.setInterval(() => {
      if (disposed) return
      const now = Date.now()
      if (now - lastNowEventAt > 6500) {
        void (async () => {
          try {
            const s = await nowPlayingSnapshot({ includeThumbnail: false, maxThumbnailBytes: 0, maxSessions: 1 })
            const np = (s as any)?.now_playing
            if (!np) return
            applyNowPlaying({
              supported: !!(s as any)?.supported,
              app_id: np.app_id ?? null,
              playback_status: np.playback_status ?? null,
              title: np.title ?? null,
              artist: np.artist ?? null,
              album_title: np.album_title ?? null,
              position_ms: np.position_ms ?? null,
              duration_ms: np.duration_ms ?? null,
              retrieved_at_unix_ms: typeof (s as any)?.retrieved_at_unix_ms === 'number' ? (s as any).retrieved_at_unix_ms : Date.now(),
              genres: np.genres ?? [],
              song_id: np.song_id ?? null
            })
          } catch {
            // ignore
          }
        })()
      }
      if (now - lastLyricsEventAt > 6500) {
        void (async () => {
          try {
            const cur = (await invoke('lyrics_get_current')) as LyricsSearchResult | null
            applyLyrics(cur)
          } catch {
            // ignore
          }
        })()
      }
    }, 2500)
    stopPoll = () => window.clearInterval(poll)

    let raf = 0
    const tick = () => {
      raf = requestAnimationFrame(tick)
      if (!timeline || timeline.lines.length === 0) return
      const playing = (nowPlaying?.playback_status || '').toLowerCase() === 'playing'
      if (!playing) return
      const pos = effectivePositionMs()
      const a = getActiveLine(timeline, pos)
      activeIndex = a.index
    }
    raf = requestAnimationFrame(tick)
    stopAnim = () => cancelAnimationFrame(raf)

    return cleanup
  })
</script>

<div class={fading ? 'root fading' : 'root'} bind:this={rootEl}>
  <canvas class="bg" bind:this={bgCanvas}></canvas>
  <canvas class="snow" bind:this={snowCanvas}></canvas>
  {#if !clickThrough}
    <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
    <div class="titlebar" data-tauri-drag-region on:mousedown|preventDefault={() => void startDrag()}>
      <div class="titlebar-title">{nowPlaying?.title || item?.title || '歌词'}</div>
      <button
        class="titlebar-btn"
        title="关闭"
        on:mousedown|stopPropagation
        on:click={() => void win?.close()}
      >
        ×
      </button>
    </div>
  {/if}
  <div class="panel">
    {#if !timeline || timeline.lines.length === 0}
      <div class="line strong">暂无歌词</div>
    {:else}
      {#each currentLines() as l, idx (idx)}
        <div class={l.strong ? 'line strong' : 'line'}>
          <div class="orig">{l.text}</div>
          {#if l.trans}
            <div class="trans">{l.trans}</div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  {#if !clickThrough}
    <div class="hint">拖动上方标题栏移动 · F2: 点击穿透 · Esc: 关闭</div>
  {/if}
</div>

<style>
  .root {
    height: 100%;
    padding: 12px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 8px;
    transition: opacity 220ms ease;
    position: relative;
    overflow: hidden;
  }

  .root.fading {
    opacity: 0.35;
  }

  .bg,
  .snow {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .bg {
    opacity: 0.9;
  }

  .titlebar {
    position: absolute;
    left: 10px;
    right: 10px;
    top: 8px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    z-index: 2;
    user-select: none;
    pointer-events: auto;
  }

  .titlebar-title {
    font-size: 11px;
    opacity: 0.7;
    color: rgba(255, 255, 255, 0.92);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .titlebar-btn {
    width: 26px;
    height: 24px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(0, 0, 0, 0.25);
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    line-height: 1;
    font-size: 18px;
  }

  .titlebar-btn:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .panel {
    border-radius: 16px;
    padding: 12px 14px;
    margin-top: 14px;
    background: rgba(0, 0, 0, 0.38);
    color: rgba(255, 255, 255, 0.94);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    position: relative;
  }

  .line {
    opacity: 0.85;
  }

  .strong {
    opacity: 1;
  }

  .orig {
    font-size: 20px;
    line-height: 1.25;
    font-weight: 700;
    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.25);
  }

  .trans {
    margin-top: 6px;
    font-size: 14px;
    opacity: 0.85;
  }

  .hint {
    font-size: 11px;
    opacity: 0.6;
    color: rgba(255, 255, 255, 0.92);
    text-align: center;
    user-select: none;
  }
</style>
