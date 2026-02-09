<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  let busy = false
  let includeThumbnail = false
  let outText = ''

  async function fetchNowPlaying() {
    busy = true
    outText = '正在获取...'
    try {
      const s = await invoke<string>('now_playing_snapshot', {
        include_thumbnail: includeThumbnail,
        max_thumbnail_bytes: 262_144,
        max_sessions: 32
      })
      outText = s || '(empty)'
    } catch (e) {
      outText = `获取失败：${String(e)}`
    } finally {
      busy = false
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
        <label class="row gap-8 align-center">
          <input type="checkbox" bind:checked={includeThumbnail} disabled={busy} />
          <span class="text-secondary">包含封面（base64）</span>
        </label>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button class="w-160" appearance="accent" disabled={busy} on:click={fetchNowPlaying}>
          {busy ? '处理中...' : '获取正在播放信息'}
        </fluent-button>
      </div>
    </div>
  </fluent-card>

  <div class="panel mono-panel">
    <pre class="mono">{outText}</pre>
  </div>
</div>

<style>
  .mono-panel {
    flex: 1;
    min-height: 260px;
  }

  .mono {
    margin: 0;
    flex: 1;
    min-height: 0;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
    color: var(--text-secondary);
  }

  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
</style>

