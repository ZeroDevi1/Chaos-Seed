<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import { formatForDisplay, type DisplayRow } from '@/shared/lyricsFormat'
  import type { LyricsSearchResult } from '@/shared/types'

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null
  let item: LyricsSearchResult | null = null
  let rows: DisplayRow[] = []

  function applyItem(next: LyricsSearchResult | null) {
    item = next
    rows = formatForDisplay(item)
  }

  async function closeSelf() {
    if (!win) return
    try {
      await win.close()
    } catch {
      // ignore
    }
  }

  onMount(() => {
    let disposed = false
    let un: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let onUnload: (() => void) | undefined

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    const cleanup = () => {
      disposed = true
      un?.()
      stopKey?.()
      if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
    }
    onUnload = () => cleanup()
    window.addEventListener('beforeunload', onUnload, true)

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') void closeSelf()
    }
    window.addEventListener('keydown', onKey, true)
    stopKey = () => window.removeEventListener('keydown', onKey, true)

    void (async () => {
      try {
        const cur = (await invoke('lyrics_get_current')) as LyricsSearchResult | null
        if (!disposed) applyItem(cur)
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const u = await listen<LyricsSearchResult | null>('lyrics_current_changed', (e) => {
          applyItem((e as unknown as { payload: LyricsSearchResult | null }).payload)
        })
        if (disposed) return u()
        un = u
      } catch {
        // ignore
      }
    })()

    return cleanup
  })
</script>

<div class="root">
  <div class="head">
    <div class="title">
      {item?.title || '-'} {item?.artist ? `- ${item.artist}` : ''}
    </div>
    <div class="meta text-secondary">
      {item?.service ? `source=${item.service}` : ''} {item ? `quality=${item.quality.toFixed(4)}` : ''}
    </div>
  </div>

  <div class="body">
    {#if rows.length === 0}
      <div class="empty text-secondary">暂无歌词。</div>
    {:else}
      <div class="rows">
        {#each rows as r, idx (idx)}
          {#if r.isMeta}
            <div class="line meta">{r.original}</div>
          {:else}
            <div class="line orig">{r.original}</div>
            {#if r.translation}
              <div class="line trans">{r.translation}</div>
            {/if}
          {/if}
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
  }
  .head {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-color);
  }
  .title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .meta {
    margin-top: 4px;
    font-size: 12px;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px;
  }
  .rows {
    white-space: pre-wrap;
    word-break: break-word;
  }
  .line {
    line-height: 1.5;
    font-size: 13px;
    color: var(--text-primary);
  }
  .meta {
    color: var(--text-secondary);
  }
  .trans {
    color: var(--text-secondary);
    margin-bottom: 6px;
  }
  .orig {
    margin-top: 6px;
  }
  .empty {
    padding: 12px;
  }
</style>
