<script lang="ts">
  import { nowPlayingSnapshot } from '@/shared/nowPlayingApi'
  import { lyricsSearch } from '@/shared/lyricsApi'
  import { openLyricsWindow, setLyricsWindowPayload, type LyricsWindowMode } from '@/shared/lyricsWindowApi'
  import { formatForDisplay, type DisplayRow } from '@/shared/lyricsFormat'
  import type { LyricsSearchResult, NowPlayingSnapshot } from '@/shared/types'

  let includeThumbnail = false

  let busyNowPlaying = false
  let busyLyrics = false
  let status = ''

  let snapshot: NowPlayingSnapshot | null = null

  let items: LyricsSearchResult[] = []
  let selectedKey = ''

  let windowMode: LyricsWindowMode = 'chat'

  function itemKey(it: LyricsSearchResult): string {
    // Avoid collisions across services: tokens are not guaranteed globally unique.
    return `${it.service}|${it.service_token}`
  }

  $: selectedItem = items.find((it) => itemKey(it) === selectedKey) ?? null
  $: displayRows = formatForDisplay(selectedItem)

  async function fetchNowPlayingAndSearch() {
    busyNowPlaying = true
    status = '正在获取正在播放信息...'
    snapshot = null
    items = []
    selectedKey = ''
    try {
      const s = await nowPlayingSnapshot({
        includeThumbnail,
        maxThumbnailBytes: 262_144,
        // Avoid returning a huge payload; this page only needs the picked now_playing.
        maxSessions: 1
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
      const title = (np.title ?? '').toString().trim()
      const artist = (np.artist ?? '').toString().trim()
      const album = (np.album_title ?? '').toString().trim()
      const durationMs = typeof np.duration_ms === 'number' ? np.duration_ms : null
      if (!title) {
        status = '正在播放信息缺少 title，无法搜索歌词。'
        return
      }

      await doLyricsSearch({ title, artist: artist || null, album: album || null, durationMs })
    } catch (e) {
      status = `获取失败：${String(e)}`
    } finally {
      busyNowPlaying = false
    }
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
        servicesCsv: 'netease,qq,kugou',
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

  async function showLyricsWindow() {
    if (!selectedItem) {
      status = '请先在中间列表选择一条歌词。'
      return
    }
    try {
      // Set payload first so already-open windows update; newly-open windows will also read the latest payload on mount.
      await setLyricsWindowPayload(selectedItem)
      await openLyricsWindow(windowMode)
    } catch (e) {
      status = `打开窗口失败：${String(e)}`
    }
  }

  function onModeChange(ev: Event) {
    windowMode = ((ev.target as unknown as { value: string })?.value ?? 'chat').toString() as LyricsWindowMode
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
        <label class="row gap-8 align-center">
          <input type="checkbox" bind:checked={includeThumbnail} disabled={busyNowPlaying} />
          <span class="text-secondary">包含封面（base64）</span>
        </label>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-180" appearance="accent" disabled={busyNowPlaying} on:click={fetchNowPlayingAndSearch}>
          {busyNowPlaying ? '处理中...' : '获取正在播放并搜索歌词'}
        </fluent-button>

        <div class="row gap-8 align-center">
          <div class="text-secondary">显示到</div>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <fluent-select class="select w-220" value={windowMode} on:change={onModeChange}>
            <fluent-option value="chat">Chat 窗口（不透明）</fluent-option>
            <fluent-option value="overlay">桌面歌词 Overlay（透明）</fluent-option>
          </fluent-select>
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <fluent-button class="w-120" appearance="outline" disabled={!selectedItem} on:click={showLyricsWindow}>
            歌词显示
          </fluent-button>
        </div>
      </div>
    </div>
  </fluent-card>

  <div class="text-secondary">{status}</div>

  <div class="panel np-panel">
    {#if snapshot?.now_playing}
      <div class="np-row">
        {#if includeThumbnail && snapshot.now_playing.thumbnail?.base64}
          <img
            class="np-cover"
            alt="cover"
            src={`data:${snapshot.now_playing.thumbnail.mime};base64,${snapshot.now_playing.thumbnail.base64}`}
          />
        {:else}
          <div class="np-cover placeholder"></div>
        {/if}

        <div class="np-meta">
          <div class="np-title">{snapshot.now_playing.title || '(unknown title)'}</div>
          <div class="np-sub text-secondary">
            {snapshot.now_playing.artist || '(unknown artist)'} · {snapshot.now_playing.album_title || '(unknown album)'}
          </div>
          <div class="np-sub text-muted">
            {snapshot.now_playing.playback_status} · duration_ms={snapshot.now_playing.duration_ms ?? '-'}
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
    {#if !selectedItem}
      <div class="empty">请选择一条歌词来源以显示正文。</div>
    {:else}
      <div class="lyrics-scroll">
        {#each displayRows as r, idx (idx)}
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
  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .select {
    min-width: 200px;
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
    grid-template-columns: 44px 80px 86px 1.2fr 1fr 1fr 140px;
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
