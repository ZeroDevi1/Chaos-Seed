<script lang="ts">
  import { onDestroy, onMount } from 'svelte'

  import { createDanmakuListStore, type DanmakuListStore } from '@/danmaku/store'

  export let maxItems: number = 400
  export let stickToBottom: boolean = false
  export let onOpenUrl: ((url: string) => void) | undefined = undefined

  // Two-way bound so parents can call `store.enqueue(...)` / `store.clear()`.
  export let store: DanmakuListStore | null = null

  let scrollEl: HTMLElement | null = null
  let listEl: HTMLElement | null = null
  let statusEl: HTMLElement | null = null
  let count = 0

  onMount(() => {
    if (!scrollEl || !listEl) return
    store = createDanmakuListStore({
      scrollEl,
      listEl,
      statusEl: statusEl ?? undefined,
      maxItems,
      stickToBottom,
      onOpenUrl,
      afterFlush: (c) => {
        count = c
      }
    })
  })

  onDestroy(() => {
    store?.dispose()
    store = null
  })
</script>

<div class="panel dm-panel">
  <div bind:this={statusEl} class="text-muted"></div>
  <div bind:this={scrollEl} class="dm-scroll">
    <div bind:this={listEl} class="dm-list"></div>
  </div>
</div>
