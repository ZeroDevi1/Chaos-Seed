<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import { createDanmakuListStore, type DanmakuListStore } from '@/danmaku/store'
import type { DanmakuUiMessage } from '@/shared/types'

type Exposed = {
  enqueue(msg: DanmakuUiMessage): void
  clear(): void
}

const props = withDefaults(
  defineProps<{
    maxItems?: number
    emptyText?: string
    stickToBottom?: boolean
    onOpenUrl?: (url: string) => void
  }>(),
  {
    maxItems: 400,
    emptyText: '等待弹幕...',
    stickToBottom: false
  }
)

const scrollEl = ref<HTMLElement | null>(null)
const listEl = ref<HTMLElement | null>(null)
const statusEl = ref<HTMLElement | null>(null)
const emptyEl = ref<HTMLElement | null>(null)

let store: DanmakuListStore | null = null

onMounted(() => {
  if (!scrollEl.value || !listEl.value) return
  store = createDanmakuListStore({
    scrollEl: scrollEl.value,
    listEl: listEl.value,
    statusEl: statusEl.value ?? undefined,
    maxItems: props.maxItems,
    stickToBottom: props.stickToBottom,
    onOpenUrl: props.onOpenUrl,
    afterFlush: (count) => {
      if (!emptyEl.value) return
      emptyEl.value.style.display = count > 0 ? 'none' : ''
    }
  })
})

onBeforeUnmount(() => {
  store?.dispose()
  store = null
})

function enqueue(msg: DanmakuUiMessage) {
  store?.enqueue(msg)
}

function clear() {
  store?.clear()
  if (emptyEl.value) emptyEl.value.style.display = ''
}

defineExpose<Exposed>({ enqueue, clear })
</script>

<template>
  <div class="panel dm-panel">
    <div ref="emptyEl" class="empty">{{ props.emptyText }}</div>
    <div ref="statusEl" class="text-muted" />
    <div ref="scrollEl" class="dm-scroll">
      <div ref="listEl" class="dm-list" />
    </div>
  </div>
</template>
