<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'

  import DanmakuList from '@/app/components/DanmakuList.svelte'
  import { routeStore } from '@/app/router'
  import { shouldSubscribeMainDanmaku } from '@/shared/danmakuSubscription'
  import { prefs } from '@/stores/prefs'
  import { windowPresence } from '@/stores/windowPresence'
  import type { DanmakuUiMessage } from '@/shared/types'
  import type { DanmakuListStore } from '@/danmaku/store'

  let input = ''
  let connected = false
  let connStatus = ''

  let listStore: DanmakuListStore | null = null

  $: emptyText = connected ? '已连接：等待弹幕...' : '请先输入直播间地址并点击“解析/连接”。'

  function applyConnectedFromStatus(s: string) {
    const t = (s || '').toString()
    if (t.includes('已连接')) connected = true
    if (t.includes('已断开')) connected = false
  }

  async function openUrl(url: string) {
    try {
      await invoke('open_url', { url })
    } catch {
      // ignore
    }
  }

  async function doConnect() {
    connStatus = ''
    try {
      await invoke('danmaku_connect', { input })
      connected = true
    } catch (e) {
      connected = false
      connStatus = `连接失败：${String(e)}`
    }
  }

  async function doDisconnect() {
    try {
      await invoke('danmaku_disconnect')
    } catch (e) {
      connStatus = `断开失败：${String(e)}`
    } finally {
      connected = false
      listStore?.clear()
    }
  }

  async function openChatWindow() {
    try {
      await invoke('open_chat_window')
      // Best-effort: immediately stop pushing high-frequency messages to the main window.
      // The window presence event may arrive slightly later; this avoids "main still renders" perception.
      void invoke('danmaku_set_msg_subscription', { enabled: false }).catch(() => {})
      const mode = get(prefs).backdropMode
      void invoke('set_backdrop', { mode }).catch(() => {})
    } catch {
      // ignore
    }
  }

  async function openOverlayWindow() {
    try {
      await invoke('open_overlay_window', { opaque: get(prefs).overlayMode === 'opaque' })
      // Best-effort: immediately stop pushing high-frequency messages to the main window.
      void invoke('danmaku_set_msg_subscription', { enabled: false }).catch(() => {})
    } catch {
      // ignore
    }
  }

  function onInput(ev: Event) {
    input = ((ev.target as unknown as { value: string })?.value ?? '').toString()
  }

  function onKeyDown(ev: KeyboardEvent) {
    if (ev.key === 'Enter') void doConnect()
  }

  onMount(() => {
    let disposed = false
    let unStatus: (() => void) | undefined
    let unMsg: (() => void) | undefined
    let unRoute: (() => void) | undefined
    let unPresence: (() => void) | undefined
    let subscribed: boolean | null = null

    let routePath = ''
    let chatOpen = false
    let overlayOpen = false

    const setSubscription = (enabled: boolean) => {
      if (subscribed === enabled) return
      subscribed = enabled
      void invoke('danmaku_set_msg_subscription', { enabled }).catch(() => {})
    }

    const recompute = () => {
      const want = shouldSubscribeMainDanmaku({ routePath, chatOpen, overlayOpen })
      setSubscription(want)
    }

    unRoute = routeStore.subscribe((s) => {
      routePath = s.path
      recompute()
    })

    unPresence = windowPresence.subscribe((s) => {
      chatOpen = s.chatOpen
      overlayOpen = s.overlayOpen
      recompute()
    })

    void (async () => {
      try {
        const un = await listen<string>('danmaku_status', (e) => {
          connStatus = e.payload
          applyConnectedFromStatus(e.payload)
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

    return () => {
      disposed = true
      unStatus?.()
      unMsg?.()
      unRoute?.()
      unPresence?.()
      // Best-effort: don't keep pushing high-frequency events to the main window once the app is closing.
      setSubscription(false)
    }
  })
</script>

<div class="page page-wide">
  <h2 class="heading">弹幕</h2>
  <div class="text-secondary">输入直播间 URL（B 站/斗鱼/虎牙）后点击“解析/连接”，页面下方会以 Chat 方式实时输出弹幕。</div>

  <fluent-card class="app-card">
    <div class="card-pad stack gap-12">
      <div class="row gap-12 wrap align-center">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <fluent-text-field
          class="input dm-url"
          placeholder=""
          value={input}
          on:input={onInput}
          on:keydown={onKeyDown}
        ></fluent-text-field>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button appearance="accent" class="w-120" disabled={connected} on:click={doConnect}>
          {connected ? '已连接' : '解析/连接'}
        </fluent-button>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button appearance="outline" class="w-92" disabled={!connected} on:click={doDisconnect}>
          断开
        </fluent-button>
      </div>

      <div class="row gap-12 wrap align-center">
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button appearance="outline" disabled={!connected} on:click={openChatWindow}>Chat 窗口</fluent-button>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button appearance="outline" disabled={!connected} on:click={openOverlayWindow}>
          Overlay 窗口
        </fluent-button>
      </div>

      <div class="text-secondary">{connStatus}</div>
    </div>
  </fluent-card>

  <DanmakuList bind:store={listStore} emptyText={emptyText} maxItems={400} onOpenUrl={openUrl} />
</div>
