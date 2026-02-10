<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { PhysicalPosition, currentMonitor } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'

  import type { LyricsSearchResult, NowPlayingSession } from '@/shared/types'
  import { getActiveLine, parseLrc, type Timeline } from '@/shared/lyricsSync'
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'
  import { FluidBackgroundEffect } from '@/app/lyrics/effects/fluidBackground'
  import { SnowParticlesEffect } from '@/app/lyrics/effects/snowParticles'
  import { Fan3DLayoutEffect } from '@/app/lyrics/effects/fan3dLayout'
  import type { BackgroundEffect, LayoutEffect, ParticleEffect } from '@/app/lyrics/effects/types'

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

  let bgCanvas: HTMLCanvasElement | null = null
  let snowCanvas: HTMLCanvasElement | null = null
  let lineEls: HTMLElement[] = []
  let bgEffect: BackgroundEffect | null = null
  let particleEffect: ParticleEffect | null = null
  let layoutEffect: LayoutEffect | null = null
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
    const playing = (nowPlaying.playback_status || '').toLowerCase() === 'playing'
    bgEffect?.setActive(playing)
    particleEffect?.setActive(playing)
    lastNowEventAt = Date.now()
  }

  async function startDragAndSnap() {
    if (!win) return
    try {
      // startDragging resolves after the drag ends.
      await win.startDragging()
    } catch {
      return
    }
    try {
      const [pos, size, mon] = await Promise.all([win.outerPosition(), win.outerSize(), currentMonitor()])
      if (!mon) return
      const margin = 6
      const monLeft = mon.position.x
      const monTop = mon.position.y
      const monRight = mon.position.x + mon.size.width
      const monBottom = mon.position.y + mon.size.height

      const centerX = pos.x + Math.floor(size.width / 2)
      const leftDist = Math.abs(centerX - monLeft)
      const rightDist = Math.abs(monRight - centerX)
      const snapLeft = leftDist <= rightDist
      const x = snapLeft ? monLeft + margin : monRight - size.width - margin
      const yMin = monTop + margin
      const yMax = Math.max(yMin, monBottom - size.height - margin)
      const y = Math.min(yMax, Math.max(yMin, pos.y))
      await win.setPosition(new PhysicalPosition(x, y))
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

  function viewLines(): Array<{ gi: number; text: string; trans: string | null; active: boolean }> {
    const tl = timeline?.lines || []
    if (tl.length === 0) return []
    const idx = activeIndex >= 0 ? activeIndex : 0
    const radius = 4
    const from = Math.max(0, idx - radius)
    const to = Math.min(tl.length - 1, idx + radius)
    const out: Array<{ gi: number; text: string; trans: string | null; active: boolean }> = []
    for (let i = from; i <= to; i++) {
      const l = tl[i]
      out.push({ gi: i, text: l.text, trans: l.translationText ?? null, active: i === idx })
    }
    return out
  }

  onMount(() => {
    let disposed = false
    let unLyrics: (() => void) | undefined
    let unNow: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopResize: (() => void) | undefined
    let stopPoll: (() => void) | undefined

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
      layoutEffect?.dispose()
    }
    window.addEventListener('beforeunload', cleanup, { capture: true, once: true })

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') void win?.close()
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
        const layout = s?.effects?.layout_effect || 'none'
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
        if (layout === 'fan3d') {
          layoutEffect = new Fan3DLayoutEffect()
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

    // Fallback: try to fetch once if the backend doesn't push state yet.
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

    // Animation loop: compute active line from interpolated position.
    let raf = 0
    const tick = () => {
      raf = requestAnimationFrame(tick)
      if (!timeline || timeline.lines.length === 0) return
      const playing = (nowPlaying?.playback_status || '').toLowerCase() === 'playing'
      if (!playing) return
      const pos = effectivePositionMs()
      const a = getActiveLine(timeline, pos)
      activeIndex = a.index

      if (layoutEffect) {
        // Apply to currently rendered lines only.
        const subset = viewLines()
        const activeLocal = subset.findIndex((x) => x.active)
        if (activeLocal >= 0) layoutEffect.apply(lineEls.filter(Boolean), activeLocal)
      }
    }
    raf = requestAnimationFrame(tick)
    stopAnim = () => cancelAnimationFrame(raf)
    return cleanup
  })
</script>

<div class="root" bind:this={rootEl}>
  <canvas class="bg" bind:this={bgCanvas}></canvas>
  <canvas class="snow" bind:this={snowCanvas}></canvas>
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="head" data-tauri-drag-region on:mousedown|preventDefault={() => void startDragAndSnap()}>
    <div class="head-left">
      <div class="title">{nowPlaying?.title || item?.title || '歌词'}</div>
      <div class="sub">
        {nowPlaying?.artist || item?.artist || ''} {nowPlaying?.playback_status ? `· ${nowPlaying.playback_status}` : ''}
      </div>
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
    <div class="head-actions" on:mousedown|stopPropagation>
      <button class="icon-btn" title="关闭" on:click={() => void win?.close()}>×</button>
    </div>
  </div>

  <div class="body">
    {#if !timeline || timeline.lines.length === 0}
      <div class="empty">暂无歌词</div>
    {:else}
      <div class="lines">
        {#each viewLines() as l, idx (l.gi)}
          <div class={l.active ? 'line active' : 'line'} bind:this={lineEls[idx]}>
            <div class="orig">{l.text}</div>
            {#if l.trans}
              <div class="trans">{l.trans}</div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .root {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: rgba(16, 16, 16, 0.92);
    color: rgba(255, 255, 255, 0.92);
    position: relative;
    overflow: hidden;
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

  .head {
    padding: 10px 12px 8px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    user-select: none;
    position: relative;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .head-left {
    min-width: 0;
    flex: 1;
  }

  .head-actions {
    flex: 0 0 auto;
  }

  .icon-btn {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(0, 0, 0, 0.25);
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    line-height: 1;
    font-size: 18px;
  }

  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .title {
    font-weight: 700;
    font-size: 13px;
    line-height: 1.2;
  }

  .sub {
    margin-top: 4px;
    font-size: 12px;
    opacity: 0.75;
    line-height: 1.2;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 10px 12px;
    position: relative;
  }

  .empty {
    opacity: 0.75;
    font-size: 12px;
  }

  .lines {
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    perspective: 500px;
    transform-style: preserve-3d;
  }

  .line {
    opacity: 0.6;
    transition: opacity 120ms ease, transform 120ms ease;
  }

  .line.active {
    opacity: 1;
    transform: translateX(2px);
  }

  .orig {
    font-size: 15px;
    line-height: 1.35;
  }

  .trans {
    margin-top: 3px;
    font-size: 12px;
    opacity: 0.8;
  }
</style>
