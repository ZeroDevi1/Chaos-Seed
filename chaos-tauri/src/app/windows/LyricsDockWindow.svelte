<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { PhysicalPosition, currentMonitor } from '@tauri-apps/api/window'
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

  let coverUrl: string | null = null
  let lastCoverKey = ''
  let coverBusy = false

  let menuOpen = false
  let alwaysOnTop = true

  let linesEl: HTMLDivElement | null = null
  let lineEls: Array<HTMLDivElement | null> = []

  let lastNowEventAt = 0
  let lastLyricsEventAt = 0

  function payloadKey(p: NowPlayingStatePayload): string {
    return `${p.app_id || ''}|${p.title || ''}|${p.artist || ''}|${p.album_title || ''}|${p.duration_ms || 0}`
  }

  function applyLyrics(next: LyricsSearchResult | null) {
    item = next
    timeline = item ? parseLrc(item.lyrics_original, item.lyrics_translation ?? null) : null
    lineEls = []
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
    const nextKey = payloadKey(p)
    const prevKey = nowPlaying
      ? `${nowPlaying.app_id}|${nowPlaying.title || ''}|${nowPlaying.artist || ''}|${nowPlaying.album_title || ''}|${nowPlaying.duration_ms || 0}`
      : ''

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
    lastNowEventAt = Date.now()

    if (nextKey && nextKey !== prevKey) void refreshCover(nextKey)
  }

  async function refreshCover(key: string) {
    if (coverBusy) return
    if (!key || key === lastCoverKey) return
    lastCoverKey = key
    coverBusy = true
    try {
      const s = await nowPlayingSnapshot({ includeThumbnail: true, maxThumbnailBytes: 2_500_000, maxSessions: 32 })
      const thumb = (s as any)?.now_playing?.thumbnail
      if (thumb?.base64 && thumb?.mime) {
        coverUrl = `data:${thumb.mime};base64,${thumb.base64}`
      } else {
        coverUrl = null
      }
    } catch {
      coverUrl = null
    } finally {
      coverBusy = false
    }
  }

  async function startDrag() {
    if (!win) return
    try {
      await win.startDragging()
    } catch {
      // ignore
    }
  }

  async function snapTo(side: 'left' | 'right') {
    if (!win) return
    try {
      const [size, mon] = await Promise.all([win.outerSize(), currentMonitor()])
      if (!mon) return
      const margin = 6
      const monLeft = mon.position.x
      const monTop = mon.position.y
      const monRight = mon.position.x + mon.size.width
      const monBottom = mon.position.y + mon.size.height

      const x = side === 'left' ? monLeft + margin : monRight - size.width - margin
      const yMin = monTop + margin
      const yMax = Math.max(yMin, monBottom - size.height - margin)
      const y = Math.min(yMax, Math.max(yMin, monTop + margin))
      await win.setPosition(new PhysicalPosition(x, y))
    } catch {
      // ignore
    }
  }

  async function toggleAlwaysOnTop() {
    if (!win) return
    try {
      const next = !alwaysOnTop
      await win.setAlwaysOnTop(next)
      alwaysOnTop = next
    } catch {
      // ignore
    }
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

  function lineClass(i: number): string {
    if (!timeline || timeline.lines.length === 0) return 'row'
    const idx = activeIndex >= 0 ? activeIndex : 0
    const d = Math.abs(i - idx)
    if (i === idx) return 'row active'
    if (d <= 1) return 'row near'
    if (d <= 4) return 'row mid'
    return 'row far'
  }

  function scrollToActive(smooth: boolean) {
    if (!linesEl) return
    if (activeIndex < 0) return
    const el = lineEls[activeIndex]
    if (!el) return
    const target = el.offsetTop - linesEl.clientHeight / 2 + el.clientHeight / 2
    try {
      linesEl.scrollTo({ top: Math.max(0, target), behavior: smooth ? 'smooth' : 'auto' })
    } catch {
      linesEl.scrollTop = Math.max(0, target)
    }
  }

  onMount(() => {
    let disposed = false
    let unLyrics: (() => void) | undefined
    let unNow: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopPoll: (() => void) | undefined
    let stopDoc: (() => void) | undefined

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    void (async () => {
      try {
        if (win) alwaysOnTop = await win.isAlwaysOnTop()
      } catch {
        // ignore
      }
    })()

    const cleanup = () => {
      disposed = true
      unLyrics?.()
      unNow?.()
      stopKey?.()
      stopAnim?.()
      stopPoll?.()
      stopDoc?.()
    }
    window.addEventListener('beforeunload', cleanup, { capture: true, once: true })

    const onDocMouse = () => {
      if (menuOpen) menuOpen = false
    }
    document.addEventListener('mousedown', onDocMouse, false)
    stopDoc = () => document.removeEventListener('mousedown', onDocMouse, false)

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') void win?.close()
      if (ev.key === 'F1') {
        ev.preventDefault()
        menuOpen = !menuOpen
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

    // Animation loop: compute active line from interpolated position.
    let raf = 0
    let prevIdx = -2
    const tick = () => {
      raf = requestAnimationFrame(tick)
      if (!timeline || timeline.lines.length === 0) return
      const playing = (nowPlaying?.playback_status || '').toLowerCase() === 'playing'
      if (!playing) return
      const pos = effectivePositionMs()
      const a = getActiveLine(timeline, pos)
      if (a.index !== activeIndex) activeIndex = a.index
      if (a.index !== prevIdx) {
        prevIdx = a.index
        requestAnimationFrame(() => scrollToActive(true))
      }
    }
    raf = requestAnimationFrame(tick)
    stopAnim = () => cancelAnimationFrame(raf)

    return cleanup
  })
</script>

<div class="root">
  <div class="titlebar">
    <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
    <div class="drag" on:mousedown|preventDefault={() => void startDrag()}>
      {#if coverUrl}
        <img class="cover" alt="cover" src={coverUrl} />
      {:else}
        <div class="cover placeholder"></div>
      {/if}
      <div class="meta">
        <div class="title">{nowPlaying?.title || item?.title || '歌词'}</div>
        <div class="sub">
          {nowPlaying?.artist || item?.artist || ''} {nowPlaying?.playback_status ? `· ${nowPlaying.playback_status}` : ''}
        </div>
      </div>
    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
    <div class="actions" on:mousedown|stopPropagation>
      <button class="icon" title="菜单 (F1)" on:click={() => (menuOpen = !menuOpen)}>⋯</button>
      <button class="icon danger" title="关闭" on:click={() => void win?.close()}>×</button>
    </div>

    {#if menuOpen}
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <div class="menu" on:mousedown|stopPropagation>
        <button
          class={alwaysOnTop ? 'menu-item active' : 'menu-item'}
          on:click={() => {
            menuOpen = false
            void toggleAlwaysOnTop()
          }}
        >
          {alwaysOnTop ? '✓ 置顶' : '置顶'}
        </button>
        <button
          class="menu-item"
          on:click={() => {
            menuOpen = false
            void snapTo('left')
          }}
        >
          贴左
        </button>
        <button
          class="menu-item"
          on:click={() => {
            menuOpen = false
            void snapTo('right')
          }}
        >
          贴右
        </button>
        <div class="sep"></div>
        <button
          class="menu-item danger"
          on:click={() => {
            menuOpen = false
            void win?.close()
          }}
        >
          关闭窗口
        </button>
      </div>
    {/if}
  </div>

  <div class="body">
    {#if !timeline || timeline.lines.length === 0}
      <div class="empty">暂无带时间轴的歌词</div>
    {:else}
      <div class="lines" bind:this={linesEl}>
        {#each timeline.lines as l, idx (idx)}
          <div class={lineClass(idx)} bind:this={lineEls[idx]}>
            <div class="orig">{l.text}</div>
            {#if l.translationText}
              <div class="trans">{l.translationText}</div>
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
    color: rgba(255, 255, 255, 0.95);
    background: radial-gradient(circle at 20% 15%, rgba(255, 85, 170, 0.95), rgba(0, 120, 255, 0.92));
    position: relative;
    overflow: hidden;
  }

  .titlebar {
    padding: 10px 12px 10px;
    user-select: none;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(0, 0, 0, 0.12);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }

  .drag {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
    cursor: default;
  }

  .cover {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    object-fit: cover;
    border: 1px solid rgba(255, 255, 255, 0.20);
    background: rgba(0, 0, 0, 0.16);
    flex: 0 0 auto;
  }

  .cover.placeholder {
    background: rgba(0, 0, 0, 0.18);
  }

  .meta {
    min-width: 0;
    flex: 1;
  }

  .title {
    font-weight: 800;
    font-size: 18px;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    margin-top: 4px;
    font-size: 12px;
    opacity: 0.86;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 6px;
    flex: 0 0 auto;
  }

  .icon {
    width: 30px;
    height: 28px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(0, 0, 0, 0.18);
    color: rgba(255, 255, 255, 0.95);
    cursor: pointer;
    line-height: 1;
    font-size: 18px;
  }

  .icon:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .icon.danger:hover {
    background: rgba(255, 80, 80, 0.16);
    border-color: rgba(255, 80, 80, 0.45);
  }

  .menu {
    position: absolute;
    right: 12px;
    top: 52px;
    width: 160px;
    padding: 6px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(20, 20, 20, 0.72);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.35);
    z-index: 5;
  }

  .menu-item {
    width: 100%;
    text-align: left;
    padding: 9px 10px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: transparent;
    color: rgba(255, 255, 255, 0.92);
    cursor: pointer;
    font-size: 13px;
  }

  .menu-item:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .menu-item.active {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.18);
  }

  .menu-item.danger:hover {
    background: rgba(255, 80, 80, 0.16);
  }

  .sep {
    height: 1px;
    background: rgba(255, 255, 255, 0.12);
    margin: 6px 0;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 10px 12px 12px;
  }

  .empty {
    opacity: 0.85;
    font-size: 12px;
  }

  .lines {
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 6px 2px 40px;
    scroll-behavior: smooth;
  }

  .row {
    opacity: 0.55;
    filter: blur(0px);
    transition: opacity 140ms ease, filter 140ms ease, transform 140ms ease;
  }

  .row.near {
    opacity: 0.72;
  }

  .row.mid {
    opacity: 0.46;
    filter: blur(0.5px);
  }

  .row.far {
    opacity: 0.22;
    filter: blur(1.5px);
  }

  .row.active {
    opacity: 1;
    filter: blur(0px);
    transform: translateX(2px);
  }

  .orig {
    font-size: 22px;
    line-height: 1.25;
    font-weight: 900;
    text-shadow: 0 2px 6px rgba(0, 0, 0, 0.28);
  }

  .row:not(.active) .orig {
    font-size: 16px;
    font-weight: 700;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.18);
  }

  .trans {
    margin-top: 6px;
    font-size: 13px;
    opacity: 0.92;
  }
</style>
