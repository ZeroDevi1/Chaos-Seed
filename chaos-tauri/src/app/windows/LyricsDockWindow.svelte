<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { PhysicalPosition, currentMonitor } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'

  import type { LyricsSearchResult, NowPlayingSession } from '@/shared/types'
  import { getActiveLine, parseLrc, type Timeline } from '@/shared/lyricsSync'
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'
  import { resolveWebviewWindow } from '@/shared/resolveWebviewWindow'

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
  let centerPadPx = 120

  let lastNowEventAt = 0
  let lastLyricsEventAt = 0

  async function ensureWin(): Promise<ReturnType<typeof getCurrentWebviewWindow> | null> {
    if (win) return win
    win = await resolveWebviewWindow('lyrics_dock')
    return win
  }

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
    const w = await ensureWin()
    if (!w) return
    try {
      await w.startDragging()
    } catch {
      // ignore
    }
  }

  async function snapTo(side: 'left' | 'right') {
    const w = await ensureWin()
    if (!w) return
    try {
      const [size, mon] = await Promise.all([w.outerSize(), currentMonitor()])
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
      await w.setPosition(new PhysicalPosition(x, y))
    } catch {
      // ignore
    }
  }

  async function toggleAlwaysOnTop() {
    const w = await ensureWin()
    if (!w) return
    try {
      const next = !alwaysOnTop
      await w.setAlwaysOnTop(next)
      alwaysOnTop = next
    } catch {
      // ignore
    }
  }

  async function closeSelf() {
    const w = await ensureWin()
    if (!w) return
    try {
      await w.close()
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

	  function updateCenterPad() {
	    if (!linesEl) return
	    const minPad = 80
	    const maxPad = 420
	    const next = Math.floor(linesEl.clientHeight / 2) - 40
	    centerPadPx = Math.max(minPad, Math.min(maxPad, next))
	    requestAnimationFrame(() => scrollToActive(false))
	  }

  onMount(() => {
    let disposed = false
    let unLyrics: (() => void) | undefined
    let unNow: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopPoll: (() => void) | undefined
    let stopDoc: (() => void) | undefined
    let stopResize: (() => void) | undefined

    void (async () => {
      win = await ensureWin()
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
      stopResize?.()
    }
    window.addEventListener('beforeunload', cleanup, { capture: true, once: true })

    const onDocMouse = () => {
      if (menuOpen) menuOpen = false
    }
    document.addEventListener('mousedown', onDocMouse, false)
    stopDoc = () => document.removeEventListener('mousedown', onDocMouse, false)

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') void closeSelf()
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

    const onResize = () => updateCenterPad()
    window.addEventListener('resize', onResize, true)
    stopResize = () => window.removeEventListener('resize', onResize, true)
    requestAnimationFrame(() => updateCenterPad())

    // Animation loop: compute active line from interpolated position.
    let raf = 0
    let prevIdx = -2
	    const tick = () => {
	      raf = requestAnimationFrame(tick)
	      if (!timeline || timeline.lines.length === 0) return
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
    <div class="drag" data-tauri-drag-region on:mousedown={() => void startDrag()}>
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
      <fluent-button class="icon" appearance="stealth" title="菜单 (F1)" on:click={() => (menuOpen = !menuOpen)}>⋯</fluent-button>
      <fluent-button class="icon danger" appearance="stealth" title="关闭" on:click={() => void closeSelf()}>×</fluent-button>
    </div>

    {#if menuOpen}
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <fluent-menu class="menu" on:mousedown|stopPropagation>
        <fluent-menu-item
          on:click={() => {
            menuOpen = false
            void toggleAlwaysOnTop()
          }}
        >
          {alwaysOnTop ? '✓ 置顶' : '置顶'}
        </fluent-menu-item>
        <fluent-menu-item
          on:click={() => {
            menuOpen = false
            void snapTo('left')
          }}
        >
          贴左
        </fluent-menu-item>
        <fluent-menu-item
          on:click={() => {
            menuOpen = false
            void snapTo('right')
          }}
        >
          贴右
        </fluent-menu-item>
      </fluent-menu>
    {/if}
  </div>

  <div class="body">
    {#if !timeline || timeline.lines.length === 0}
      <div class="empty">暂无带时间轴的歌词</div>
    {:else}
      <div class="lines" bind:this={linesEl} style:--center-pad={`${centerPadPx}px`}>
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
    color: var(--text-primary);
    background: var(--panel-bg);
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
    border-bottom: 1px solid var(--border-color);
    background: var(--card-bg);
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
    border: 1px solid var(--border-color);
    background: var(--hover-bg);
    flex: 0 0 auto;
  }

  .cover.placeholder {
    background: var(--hover-bg);
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
    color: var(--text-secondary);
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
  }

  .icon::part(control) {
    width: 30px;
    height: 28px;
    border-radius: 10px;
    border: 1px solid var(--border-color);
    background: var(--button-secondary-bg);
    color: var(--button-secondary-fg);
    line-height: 1;
    font-size: 18px;
    padding: 0;
    justify-content: center;
  }

  .icon::part(control):hover {
    background: var(--button-secondary-bg-hover);
  }

  .icon.danger::part(control):hover {
    background: color-mix(in srgb, #ff5050 18%, var(--button-secondary-bg-hover));
  }

  .menu {
    position: absolute;
    right: 12px;
    top: 52px;
    width: 180px;
    z-index: 5;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 10px 12px 12px;
  }

  .empty {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .lines {
    height: 100%;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: var(--center-pad, 120px) 2px var(--center-pad, 120px);
    scroll-behavior: smooth;
  }

  .row {
    opacity: 0.68;
    transition: opacity 140ms ease, transform 140ms ease, background 140ms ease;
    padding: 6px 8px;
    border-radius: 10px;
  }

  .row.near {
    opacity: 0.78;
  }

  .row.mid {
    opacity: 0.70;
  }

  .row.far {
    opacity: 0.66;
  }

  .row.active {
    opacity: 1;
    transform: translateX(2px);
    background: var(--selected-bg);
  }

  .orig {
    font-size: 20px;
    line-height: 1.25;
    font-weight: 900;
  }

  .row:not(.active) .orig {
    font-size: 16px;
    font-weight: 800;
  }

  .trans {
    margin-top: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
</style>
