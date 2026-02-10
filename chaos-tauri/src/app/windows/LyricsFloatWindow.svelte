<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import type { LyricsSearchResult, NowPlayingSession } from '@/shared/types'
  import { getActiveLine, parseLrc, type Timeline } from '@/shared/lyricsSync'
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'

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

  let item: LyricsSearchResult | null = null
  let timeline: Timeline | null = null
  let activeIndex = -1

  let nowPlaying: NowPlayingSession | null = null
  let nowRetrievedAtMs = 0

  let clickThrough = false
  let fading = false

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
    lastNowEventAt = Date.now()
  }

  function effectivePositionMs(): number {
    if (!nowPlaying) return 0
    const pos = typeof nowPlaying.position_ms === 'number' ? nowPlaying.position_ms : 0
    const dur = typeof nowPlaying.duration_ms === 'number' ? nowPlaying.duration_ms : null
    const playing = (nowPlaying.playback_status || '').toLowerCase() === 'playing'
    if (!playing) return pos
    const dt = Date.now() - nowRetrievedAtMs
    const v = pos + Math.max(0, dt)
    return dur != null ? Math.min(v, dur) : v
  }

  function viewText(): { line1: string; line2: string | null } {
    const tl = timeline?.lines || []
    if (tl.length === 0) return { line1: '暂无歌词', line2: null }
    const idx = activeIndex >= 0 ? activeIndex : 0
    const cur = tl[idx]
    const second = cur.translationText ?? tl[idx + 1]?.text ?? null
    return { line1: cur.text || '', line2: second && second.trim() ? second : null }
  }

  $: view = viewText()

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
      stopPoll?.()
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
        const s = await nowPlayingSnapshot({ includeThumbnail: false, maxThumbnailBytes: 0, maxSessions: 32 })
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
            const s = await nowPlayingSnapshot({ includeThumbnail: false, maxThumbnailBytes: 0, maxSessions: 32 })
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

<div class={fading ? 'root fading' : 'root'}>
  <div class="bar">
    <div class="text">
      <div class="line1">{view.line1}</div>
      {#if view.line2}
        <div class="line2">{view.line2}</div>
      {/if}
    </div>

    {#if !clickThrough}
      <div class="actions">
        <button class="btn" title="点击穿透 (F2)" on:click={() => void applyClickThrough(true)}>⤧</button>
        <button class="btn danger" title="关闭" on:click={() => void win?.close()}>×</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .root {
    height: 100%;
    width: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6px 10px;
    transition: opacity 220ms ease;
  }

  .root.fading {
    opacity: 0.35;
  }

  .bar {
    width: 100%;
    height: 100%;
    border-radius: 12px;
    padding: 10px 12px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    background: rgba(0, 0, 0, 0.42);
    color: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.14);
  }

  .text {
    min-width: 0;
    flex: 1;
  }

  .line1 {
    font-size: 18px;
    font-weight: 800;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-shadow: 0 2px 6px rgba(0, 0, 0, 0.24);
  }

  .line2 {
    margin-top: 6px;
    font-size: 13px;
    font-weight: 600;
    opacity: 0.86;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 8px;
    flex: 0 0 auto;
  }

  .btn {
    width: 34px;
    height: 30px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(0, 0, 0, 0.20);
    color: rgba(255, 255, 255, 0.92);
    cursor: pointer;
    line-height: 1;
    font-size: 16px;
  }

  .btn:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .btn.danger:hover {
    background: rgba(255, 80, 80, 0.16);
    border-color: rgba(255, 80, 80, 0.45);
  }
</style>
