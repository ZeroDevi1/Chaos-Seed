<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import DanmakuList from '@/app/components/DanmakuList.vue'
import { usePrefsStore } from '@/stores/prefs'
import type { DanmakuUiMessage } from '@/shared/types'

type DanmakuListExpose = {
  enqueue(msg: DanmakuUiMessage): void
  clear(): void
}

const prefs = usePrefsStore()

const input = ref('')
const connected = ref(false)
const connStatus = ref<string>('')

const listRef = ref<DanmakuListExpose | null>(null)

const emptyText = computed(() =>
  connected.value ? '已连接：等待弹幕...' : '请先输入直播间地址并点击“解析/连接”。'
)

function applyConnectedFromStatus(s: string) {
  const t = (s || '').toString()
  if (t.includes('已连接')) connected.value = true
  if (t.includes('已断开')) connected.value = false
}

async function openUrl(url: string) {
  try {
    await invoke('open_url', { url })
  } catch {
    // ignore
  }
}

async function doConnect() {
  connStatus.value = ''
  try {
    await invoke('danmaku_connect', { input: input.value })
    connected.value = true
  } catch (e) {
    connected.value = false
    connStatus.value = `连接失败：${String(e)}`
  }
}

async function doDisconnect() {
  try {
    await invoke('danmaku_disconnect')
  } catch (e) {
    connStatus.value = `断开失败：${String(e)}`
  } finally {
    connected.value = false
    listRef.value?.clear()
  }
}

async function openChatWindow() {
  try {
    await invoke('open_chat_window')
  } catch {
    // ignore
  }
}

async function openOverlayWindow() {
  try {
    await invoke('open_overlay_window', { opaque: prefs.overlayMode === 'opaque' })
  } catch {
    // ignore
  }
}

function onInput(ev: Event) {
  input.value = (ev.target as unknown as { value: string }).value
}

function onKeyDown(ev: KeyboardEvent) {
  if (ev.key === 'Enter') void doConnect()
}

let unStatus: (() => void) | undefined
let unMsg: (() => void) | undefined
let disposed = false

onMounted(() => {
  disposed = false

  void (async () => {
    try {
      const un = await listen<string>('danmaku_status', (e) => {
        connStatus.value = e.payload
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
})
</script>

<template>
  <div class="page page-wide">
    <h2 class="heading">弹幕</h2>
    <div class="text-secondary">输入直播间 URL（B 站/斗鱼/虎牙）后点击“解析/连接”，页面下方会以 Chat 方式实时输出弹幕。</div>

    <fluent-card class="app-card">
      <div class="card-pad stack gap-12">
        <div class="row gap-12 wrap align-center">
          <fluent-text-field
            class="input dm-url"
            placeholder="例如：https://live.bilibili.com/1 / https://www.douyu.com/xxx / https://www.huya.com/xxx"
            :value="input"
            @input="onInput"
            @keydown="onKeyDown"
          />
          <fluent-button appearance="accent" class="w-120" :disabled="connected" @click="doConnect">
            {{ connected ? '已连接' : '解析/连接' }}
          </fluent-button>
          <fluent-button appearance="outline" class="w-92" :disabled="!connected" @click="doDisconnect">
            断开
          </fluent-button>
        </div>

        <div class="row gap-12 wrap align-center">
          <fluent-button appearance="outline" :disabled="!connected" @click="openChatWindow">Chat 窗口</fluent-button>
          <fluent-button appearance="outline" :disabled="!connected" @click="openOverlayWindow">
            Overlay 窗口
          </fluent-button>
        </div>

        <div class="text-secondary">{{ connStatus }}</div>
      </div>
    </fluent-card>

    <DanmakuList ref="listRef" :empty-text="emptyText" :max-items="400" :on-open-url="openUrl" />
  </div>
</template>
