<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import DanmakuList from '@/app/components/DanmakuList.svelte'
  import type { DanmakuListStore } from '@/danmaku/store'
  import type { DanmakuUiMessage } from '@/shared/types'

  let connStatus = ''
  let msgCount = 0
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
          msgCount++
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

<div class="window-root">
  <div class="page" style="height: 100%">
    <div class="row gap-12 wrap align-center">
      <h2 class="heading" style="margin: 0">弹幕 - Chat ({msgCount})</h2>
      <div class="topbar-spacer"></div>
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <fluent-button appearance="outline" class="w-92" on:click={closeSelf}>关闭</fluent-button>
    </div>
    <div class="text-secondary">{connStatus}</div>
    <DanmakuList
      bind:store={listStore}
      emptyText="等待弹幕..."
      maxItems={300}
      stickToBottom={true}
      onOpenUrl={openUrl}
    />
  </div>
</div>
