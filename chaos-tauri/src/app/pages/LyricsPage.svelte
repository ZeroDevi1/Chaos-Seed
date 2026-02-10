<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'
  import { lyricsSearch } from '@/shared/lyricsApi'
  import { formatForDisplay, type DisplayRow } from '@/shared/lyricsFormat'
  import type { LyricsSearchResult, NowPlayingSnapshot } from '@/shared/types'
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { windowPresence } from '@/stores/windowPresence'

  let busyNowPlaying = false
  let busyLyrics = false
  let status = ''

  let snapshot: NowPlayingSnapshot | null = null
  let liveNowPlaying: any | null = null
  let detectionEnabled = false
  let currentItem: LyricsSearchResult | null = null
  let dockOpen = false
  let floatOpen = false

  let items: LyricsSearchResult[] = []
  let selectedKey = ''

  function itemKey(it: LyricsSearchResult): string {
    // Avoid collisions across services: tokens are not guaranteed globally unique.
    return `${it.service}|${it.service_token}`
  }

  $: selectedItem = items.find((it) => itemKey(it) === selectedKey) ?? null
  $: effectiveLyricsItem = selectedItem ?? currentItem
  $: effectiveRows = formatForDisplay(effectiveLyricsItem)

  onMount(() => {
    let unDet: (() => void) | undefined
    let unNp: (() => void) | undefined
    let unCur: (() => void) | undefined
    let unPresence: (() => void) | undefined

    void (async () => {
      try {
        const s = (await invoke('lyrics_settings_get')) as any
        detectionEnabled = !!s?.lyrics_detection_enabled
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        currentItem = ((await invoke('lyrics_get_current')) as LyricsSearchResult | null) ?? null
      } catch {
        currentItem = null
      }
    })()

    void (async () => {
      try {
        unDet = await listen<{ enabled: boolean }>('lyrics_detection_state_changed', (e) => {
          detectionEnabled = !!e.payload?.enabled
        })
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        unNp = await listen<any>('now_playing_state_changed', (e) => {
          liveNowPlaying = e.payload
        })
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        unCur = await listen<LyricsSearchResult | null>('lyrics_current_changed', (e) => {
          currentItem = (e as unknown as { payload: LyricsSearchResult | null }).payload ?? null
        })
      } catch {
        // ignore
      }
    })()

    unPresence = windowPresence.subscribe((s) => {
      dockOpen = !!(s as any)?.lyricsDockOpen
      floatOpen = !!(s as any)?.lyricsFloatOpen
    })

    // Best-effort: initial sync in case the "window opened" event was emitted before we started listening.
    void (async () => {
      try {
        dockOpen = !!(await WebviewWindow.getByLabel('lyrics_dock'))
      } catch {
        // ignore
      }
      try {
        floatOpen = !!(await WebviewWindow.getByLabel('lyrics_float'))
      } catch {
        // ignore
      }
    })()

    return () => {
      unDet?.()
      unNp?.()
      unCur?.()
      unPresence?.()
    }
  })

  async function fetchNowPlaying() {
    busyNowPlaying = true
    status = '正在获取正在播放信息...'
    try {
      const s = await nowPlayingSnapshot({
        includeThumbnail: true,
        // (core caps to 2.5MB) keep it large enough so cover art doesn't get truncated.
        maxThumbnailBytes: 2_500_000,
        // With core-side "picked session thumbnail only", this is safe and avoids missing the real playing session.
        maxSessions: 32
      })
      snapshot = s
      const np = s?.now_playing
      if (!s?.supported) {
        status = '当前平台不支持 Now Playing。'
        return
      }
      if (!np) {
        status = '未检测到正在播放的媒体会话。'
        return
      }
      status = '已获取正在播放信息。'
    } catch (e) {
      status = `获取失败：${String(e)}`
    } finally {
      busyNowPlaying = false
    }
  }

  async function searchLyricsFromNowPlaying() {
    const np = snapshot?.now_playing ?? liveNowPlaying
    if (!np) {
      status = '请先点击“获取正在播放”。'
      return
    }
    const title = (np.title ?? '').toString().trim()
    const artist = (np.artist ?? '').toString().trim()
    const album = (np.album_title ?? '').toString().trim()
    const durationMs = typeof np.duration_ms === 'number' ? np.duration_ms : null
    if (!title) {
      status = '正在播放信息缺少 title，无法搜索歌词。'
      return
    }
    await doLyricsSearch({ title, artist: artist || null, album: album || null, durationMs })
  }

  async function doLyricsSearch(input: {
    title: string
    artist: string | null
    album: string | null
    durationMs: number | null
  }) {
    busyLyrics = true
    status = '正在搜索歌词...'
    items = []
    selectedKey = ''
    try {
      const out = await lyricsSearch({
        title: input.title,
        artist: input.artist,
        album: input.album,
        durationMs: input.durationMs,
        limit: 10,
        strictMatch: false,
        servicesCsv: 'qq,netease,lrclib',
        timeoutMs: 8000
      })
      items = out || []
      status = `搜索完成：${items.length} 条结果`
      if (items.length > 0) selectedKey = itemKey(items[0])
    } catch (e) {
      items = []
      status = `搜索失败：${String(e)}`
    } finally {
      busyLyrics = false
    }
  }

  async function toggleDetection() {
    try {
      const next = !detectionEnabled
      await invoke('lyrics_detection_set_enabled', { enabled: next })
      detectionEnabled = next
    } catch (e) {
      status = `切换失败：${String(e)}`
    }
  }

  async function openDock() {
    try {
      await invoke('open_lyrics_dock_window')
    } catch (e) {
      status = `打开窄屏歌词窗失败：${String(e)}`
    }
  }

  async function openFloat() {
    try {
      await invoke('open_lyrics_float_window')
    } catch (e) {
      status = `打开顶栏歌词失败：${String(e)}`
    }
  }

  async function closeDock() {
    try {
      const w = await WebviewWindow.getByLabel('lyrics_dock')
      await w?.close()
    } catch {
      // ignore
    }
  }

  async function closeFloat() {
    try {
      const w = await WebviewWindow.getByLabel('lyrics_float')
      await w?.close()
    } catch {
      // ignore
    }
  }

  async function toggleDockWindow() {
    try {
      const w = await WebviewWindow.getByLabel('lyrics_dock')
      if (w) {
        await w.close()
        dockOpen = false
        return
      }
    } catch {
      // ignore
    }
    await openDock()
    dockOpen = true
  }

  async function toggleFloatWindow() {
    try {
      const w = await WebviewWindow.getByLabel('lyrics_float')
      if (w) {
        await w.close()
        floatOpen = false
        return
      }
    } catch {
      // ignore
    }
    await openFloat()
    floatOpen = true
  }

  async function applySelectedAsCurrent() {
    if (!selectedItem) {
      status = '请先在中间列表选择一条歌词。'
      return
    }
    try {
      await invoke('lyrics_set_current', { payload: selectedItem })
      currentItem = selectedItem
      status = '已设置为当前歌词（Dock/Float 会自动更新）。'
    } catch (e) {
      status = `设置失败：${String(e)}`
    }
  }
</script>

<div class="page page-wide">
  <h2 class="heading">歌词</h2>
  <div class="text-secondary">
    规划：根据系统“正在播放”媒体在线搜索歌词，滚动显示，并提供类似 QQ 音乐的桌面歌词（独立置顶/Overlay）。
  </div>

  <fluent-card class="app-card">
    <div class="card-pad stack gap-12">
      <div class="row gap-12 wrap align-center">
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-160" appearance={detectionEnabled ? 'accent' : 'outline'} on:click={toggleDetection}>
          {detectionEnabled ? '歌词检测：已开启' : '歌词检测：已关闭'}
        </fluent-button>

        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-140" appearance="outline" disabled={busyNowPlaying} on:click={fetchNowPlaying}>
          {busyNowPlaying ? '处理中...' : '获取正在播放'}
        </fluent-button>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-120" appearance="accent" disabled={busyLyrics} on:click={searchLyricsFromNowPlaying}>
          {busyLyrics ? '搜索中...' : '搜索歌词'}
        </fluent-button>

        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-160" appearance="outline" disabled={!selectedItem} on:click={applySelectedAsCurrent}>
          设为当前歌词
        </fluent-button>

        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-160" appearance={dockOpen ? 'accent' : 'outline'} on:click={toggleDockWindow}>
          {dockOpen ? '窄屏歌词窗：已打开' : '窄屏歌词窗：已关闭'}
        </fluent-button>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-160" appearance={floatOpen ? 'accent' : 'outline'} on:click={toggleFloatWindow}>
          {floatOpen ? '顶栏歌词：已打开' : '顶栏歌词：已关闭'}
        </fluent-button>
      </div>
    </div>
  </fluent-card>

  <div class="text-secondary">{status}</div>

  <div class="panel np-panel">
    {#if liveNowPlaying || snapshot?.now_playing}
      <div class="np-row">
        {#if snapshot?.now_playing?.thumbnail?.base64}
          <img
            class="np-cover"
            alt="cover"
            src={`data:${snapshot.now_playing.thumbnail.mime};base64,${snapshot.now_playing.thumbnail.base64}`}
          />
        {:else}
          <div class="np-cover placeholder"></div>
        {/if}

        <div class="np-meta">
          <div class="np-title">{(liveNowPlaying?.title || snapshot?.now_playing?.title) ?? '(unknown title)'}</div>
          <div class="np-sub text-secondary">
            {(liveNowPlaying?.artist || snapshot?.now_playing?.artist) ?? '(unknown artist)'} · {(liveNowPlaying?.album_title ||
              snapshot?.now_playing?.album_title) ?? '(unknown album)'}
          </div>
          <div class="np-sub text-muted">
            {(liveNowPlaying?.playback_status || snapshot?.now_playing?.playback_status) ?? 'Unknown'} · duration_ms={(liveNowPlaying?.duration_ms ||
              snapshot?.now_playing?.duration_ms) ?? '-'}
          </div>
        </div>
      </div>
    {:else}
      <div class="empty">{busyNowPlaying ? '正在获取...' : '点击上方按钮获取正在播放信息。'}</div>
    {/if}
  </div>

  <div class="panel results-panel">
    {#if items.length === 0}
      <div class="empty">{busyLyrics ? '正在搜索...' : '暂无歌词来源列表。'}</div>
    {:else}
      <div class="results-scroll">
        <div class="result-head">
          <div class="col-pick"></div>
          <div class="col-service">来源</div>
          <div class="col-match">匹配</div>
          <div class="col-quality">质量</div>
          <div class="col-title">标题</div>
          <div class="col-artist">歌手</div>
          <div class="col-album">专辑</div>
          <div class="col-flag">标记</div>
        </div>

        <div class="results-list">
          {#each items as it (itemKey(it))}
            <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
            <div class="result-row" on:click={() => (selectedKey = itemKey(it))}>
              <div class="col-pick">
                <input type="radio" name="lyricPick" value={itemKey(it)} bind:group={selectedKey} />
              </div>
              <div class="col-service">{it.service}</div>
              <div class="col-match">{Math.round(it.match_percentage)}</div>
              <div class="col-quality">{it.quality.toFixed(4)}</div>
              <div class="col-title" title={it.title || ''}>{it.title || '-'}</div>
              <div class="col-artist" title={it.artist || ''}>{it.artist || '-'}</div>
              <div class="col-album" title={it.album || ''}>{it.album || '-'}</div>
              <div class="col-flag">
                <span class="tag">{it.matched ? 'matched' : 'unmatched'}</span>
                {#if it.has_translation}
                  <span class="tag">trans</span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="panel lyrics-panel">
    {#if !effectiveLyricsItem}
      <div class="empty">请选择一条歌词来源，或先开启检测/自动获取到歌词。</div>
    {:else}
      <div class="lyrics-head text-secondary">
        {#if selectedItem}
          正文预览（已选择）
        {:else}
          当前歌词（自动）
        {/if}
      </div>
      <div class="lyrics-scroll">
        {#each effectiveRows as r, idx (idx)}
          {#if r.isMeta}
            <div class="ly-line meta">{r.original}</div>
          {:else}
            <div class="ly-line orig">{r.original}</div>
            {#if r.translation}
              <div class="ly-line trans">{r.translation}</div>
            {/if}
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .lyrics-head {
    padding: 10px 12px 0;
    font-size: 12px;
  }

  .np-panel {
    min-height: 120px;
  }
  .np-row {
    display: flex;
    gap: 12px;
    padding: 12px;
    align-items: center;
  }
  .np-cover {
    width: 72px;
    height: 72px;
    border-radius: 10px;
    border: 1px solid var(--border-color);
    object-fit: cover;
    background: var(--card-bg);
  }
  .np-cover.placeholder {
    background: var(--input-bg);
  }
  .np-title {
    font-weight: 600;
    color: var(--text-primary);
  }
  .np-sub {
    margin-top: 4px;
    font-size: 12px;
  }

  .results-panel {
    min-height: 220px;
  }
  .results-scroll {
    padding: 10px 12px;
  }
  .result-head,
  .result-row {
    display: grid;
    grid-template-columns: 44px 80px 60px 86px 1.2fr 1fr 1fr 140px;
    gap: 8px;
    align-items: center;
  }
  .result-head {
    font-size: 12px;
    color: var(--text-secondary);
    padding: 4px 0 8px;
    border-bottom: 1px solid var(--border-color);
    margin-bottom: 8px;
  }
  .result-row {
    padding: 8px 6px;
    border-radius: 10px;
    cursor: pointer;
  }
  .result-row:hover {
    background: rgba(127, 127, 127, 0.08);
  }
  .col-title,
  .col-artist,
  .col-album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    margin-right: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .lyrics-panel {
    flex: 1;
    min-height: 240px;
  }
  .lyrics-scroll {
    padding: 12px;
    overflow: auto;
    height: 100%;
    min-height: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
    font-size: 12px;
    line-height: 1.5;
  }
  .ly-line.meta {
    color: var(--text-secondary);
  }
  .ly-line.trans {
    color: var(--text-secondary);
    margin-bottom: 6px;
  }
</style>
