<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import DanmakuList from '@/app/components/DanmakuList.vue'
import type { DanmakuUiMessage } from '@/shared/types'

type DanmakuListExpose = {
  enqueue(msg: DanmakuUiMessage): void
  clear(): void
}

const connStatus = ref<string>('')
const listRef = ref<DanmakuListExpose | null>(null)
const msgCount = ref(0)
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

let unStatus: (() => void) | undefined
let unMsg: (() => void) | undefined
let onKey: ((ev: KeyboardEvent) => void) | undefined
let disposed = false
let onUnload: (() => void) | undefined

onMounted(() => {
  disposed = false
  try {
    win = getCurrentWebviewWindow()
  } catch {
    win = null
  }

  const cleanup = () => {
    disposed = true
    unStatus?.()
    unMsg?.()
    if (onKey) window.removeEventListener('keydown', onKey, true)
    if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
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
        connStatus.value = e.payload
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
        msgCount.value++
        listRef.value?.enqueue(e.payload)
      })
      if (disposed) return un()
      unMsg = un
    } catch {
      // ignore
    }
  })()
})

onBeforeUnmount(() => {
  disposed = true
  unStatus?.()
  unMsg?.()
  if (onKey) window.removeEventListener('keydown', onKey, true)
  if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
})
</script>

<template>
  <div class="window-root">
    <div class="page" style="height: 100%">
      <div class="row gap-12 wrap align-center">
        <h2 class="heading" style="margin: 0">弹幕 - Chat ({{ msgCount }})</h2>
        <div class="topbar-spacer" />
        <fluent-button appearance="outline" class="w-92" @click="closeSelf">关闭</fluent-button>
      </div>
      <div class="text-secondary">{{ connStatus }}</div>
      <DanmakuList
        ref="listRef"
        :empty-text="'等待弹幕...'"
        :max-items="300"
        :stick-to-bottom="true"
        :on-open-url="openUrl"
      />
    </div>
  </div>
</template>
