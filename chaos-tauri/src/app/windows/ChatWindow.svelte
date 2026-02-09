<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import DanmakuList from '@/app/components/DanmakuList.svelte'
  import type { DanmakuListStore } from '@/danmaku/store'
  import type { DanmakuUiMessage } from '@/shared/types'

  let connStatus = ''
  let listStore: DanmakuListStore | null = null

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null

  async function openUrl(url: string) {
    try {
      await invoke('open_url', { url })
    } catch {
      // ignore
    }
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
    let unStatus: (() => void) | undefined
    let unMsg: (() => void) | undefined
    let onKey: ((ev: KeyboardEvent) => void) | undefined
    let onUnload: (() => void) | undefined

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    // Prefer Chat as the primary renderer when it is open (main window will unsubscribe).
    void invoke('danmaku_set_msg_subscription', { enabled: true }).catch(() => {})

    const cleanup = () => {
      disposed = true
      unStatus?.()
      unMsg?.()
      if (onKey) window.removeEventListener('keydown', onKey, true)
      if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
      void invoke('danmaku_set_msg_subscription', { enabled: false }).catch(() => {})
    }

    onUnload = () => cleanup()
    window.addEventListener('beforeunload', onUnload, true)

    onKey = (ev) => {
      if (ev.key === 'Escape') void closeSelf()
    }
    window.addEventListener('keydown', onKey, true)

    void (async () => {
      try {
        const un = await listen<string>('danmaku_status', (e) => {
          connStatus = e.payload
        })
        if (disposed) return un()
        unStatus = un
      } catch {
        // ignore
      }
    })()

    void (async () => {
      try {
        const un = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
          listStore?.enqueue(e.payload)
        })
        if (disposed) return un()
        unMsg = un
      } catch {
        // ignore
      }
    })()

    return cleanup
  })
</script>

<div class="chat-root">
  {#if connStatus}
    <div class="chat-status text-secondary">{connStatus}</div>
  {/if}
  <div class="chat-body">
    <DanmakuList
      bind:store={listStore}
      emptyText="等待弹幕..."
      maxItems={300}
      stickToBottom={true}
      onOpenUrl={openUrl}
    />
  </div>
</div>
