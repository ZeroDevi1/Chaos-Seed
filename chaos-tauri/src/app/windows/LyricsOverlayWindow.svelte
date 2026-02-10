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

    // For transparent Tauri windows, the webview background must also be transparent.
    document.documentElement.style.background = 'transparent'
    document.body.style.background = 'transparent'

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
        const u = await listen<LyricsSearchResult>('lyrics_current_changed', (e) => {
          applyItem(e.payload)
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
  <div class="pad">
    <div class="title">{item?.title || ''} {item?.artist ? `- ${item.artist}` : ''}</div>
    <div class="rows">
      {#each rows as r, idx (idx)}
        {#if r.isMeta}
          <!-- skip meta in overlay -->
        {:else}
          <div class="line orig">{r.original}</div>
          {#if r.translation}
            <div class="line trans">{r.translation}</div>
          {/if}
        {/if}
      {/each}
    </div>
  </div>
</div>

<style>
  .root {
    height: 100%;
    padding: 12px;
    box-sizing: border-box;
  }

  .pad {
    height: 100%;
    border-radius: 14px;
    padding: 12px 14px;
    background: rgba(0, 0, 0, 0.35);
    color: rgba(255, 255, 255, 0.92);
    overflow: auto;
  }

  .title {
    font-weight: 600;
    font-size: 16px;
    line-height: 1.2;
    margin-bottom: 10px;
    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.2);
  }

  .rows {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .line {
    font-size: 16px;
    line-height: 1.45;
  }

  .trans {
    opacity: 0.85;
    margin-bottom: 8px;
  }
</style>

