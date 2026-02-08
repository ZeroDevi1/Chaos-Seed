<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { DanmakuUiMessage } from '@/shared/types'
import { usePrefsStore } from '@/stores/prefs'

type Sprite = {
  text: string
  x: number
  y: number
  w: number
  speedPxPerSec: number
}

const prefs = usePrefsStore()

const params = new URLSearchParams(window.location.search)
const opaque = params.get('overlay') === 'opaque'

const canvasEl = ref<HTMLCanvasElement | null>(null)
const msgCount = ref(0)
let win: ReturnType<typeof getCurrentWebviewWindow> | null = null

let stopThemeWatch: (() => void) | undefined
let unMsg: (() => void) | undefined
let stopResize: (() => void) | undefined
let stopAnim: (() => void) | undefined
let stopKey: (() => void) | undefined
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
    stopAnim?.()
    stopResize?.()
    stopThemeWatch?.()
    stopKey?.()
    unMsg?.()
    if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
  }
  onUnload = () => cleanup()
  window.addEventListener('beforeunload', onUnload, true)

  if (!opaque) {
    // For transparent Tauri windows, the webview background must also be transparent.
    document.documentElement.style.background = 'transparent'
    document.body.style.background = 'transparent'
  }

  const onKey = (ev: KeyboardEvent) => {
    if (ev.key === 'Escape') void win?.close()
  }
  window.addEventListener('keydown', onKey, true)
  stopKey = () => window.removeEventListener('keydown', onKey, true)

  const canvas = canvasEl.value
  if (!canvas) throw new Error('canvas not found')
  // TS doesn't reliably narrow captured variables inside nested functions; capture a non-null handle.
  const canvas2: HTMLCanvasElement = canvas

  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas 2d context not available')
  const context: CanvasRenderingContext2D = ctx

  let bg = 'rgba(0,0,0,0)'
  let fg = '#ffffff'

  function refreshColors() {
    const st = getComputedStyle(document.documentElement)
    bg = st.getPropertyValue('--app-bg').trim() || '#000000'
    fg = st.getPropertyValue('--text-primary').trim() || '#ffffff'
  }
  refreshColors()

  stopThemeWatch = watch(
    () => prefs.resolvedTheme,
    () => refreshColors()
  )

  let w = 0
  let h = 0
  function resize() {
    w = Math.max(1, window.innerWidth)
    h = Math.max(1, window.innerHeight)
    const dpr = Math.max(1, window.devicePixelRatio || 1)
    canvas2.width = Math.floor(w * dpr)
    canvas2.height = Math.floor(h * dpr)
    canvas2.style.width = `${w}px`
    canvas2.style.height = `${h}px`
    context.setTransform(dpr, 0, 0, dpr, 0, 0)
  }
  resize()
  window.addEventListener('resize', resize)
  stopResize = () => window.removeEventListener('resize', resize)

  let queue: DanmakuUiMessage[] = []
  let qHead = 0
  let dropped = 0
  let lastMsgAt = 0

  const sprites: Sprite[] = []
  let lane = 0
  const laneCount = 10
  const laneHeight = 28
  const topPad = 12

  function enqueue(msg: DanmakuUiMessage) {
    queue.push(msg)
    lastMsgAt = performance.now()
    if (queue.length > 600) {
      const keep = queue.slice(-120)
      dropped += queue.length - keep.length
      queue = keep
      qHead = 0
    }
  }

  function spawn(maxPerFrame: number) {
    const pending = queue.length - qHead
    if (pending <= 0) return
    const n = Math.min(maxPerFrame, pending)
    for (let i = 0; i < n; i++) {
      const msg = queue[qHead++]!
      const shownText =
        msg.image_url && (msg.text === '[图片]' || msg.text === '[表情]') ? '[表情]' : msg.text
      const text = (shownText || '').trim()
      if (!text) continue

      context.font = '18px system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif'
      const m = context.measureText(text)
      const tw = Math.ceil(m.width)

      const y = topPad + (lane % laneCount) * laneHeight
      lane++
      const durationMs = 8000
      const distance = w + tw + 80
      const speedPxPerSec = distance / (durationMs / 1000)
      sprites.push({ text, x: w + 40, y, w: tw, speedPxPerSec })
    }

    // Compact occasionally so the array doesn't grow forever.
    if (qHead > 256 && qHead * 2 > queue.length) {
      queue = queue.slice(qHead)
      qHead = 0
    }
  }

  let stopped = false
  let raf = 0
  let lastTs = performance.now()
  function frame(ts: number) {
    if (stopped) return
    const dt = Math.min(80, Math.max(0, ts - lastTs))
    lastTs = ts

    spawn(2)

    if (opaque) {
      context.fillStyle = bg
      context.fillRect(0, 0, w, h)
    } else {
      context.clearRect(0, 0, w, h)
    }

    context.font = '18px system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif'
    context.fillStyle = fg
    context.shadowColor = 'rgba(0,0,0,0.75)'
    context.shadowBlur = 3
    context.shadowOffsetX = 0
    context.shadowOffsetY = 1
    for (let i = 0; i < sprites.length; i++) {
      sprites[i].x -= sprites[i].speedPxPerSec * (dt / 1000)
    }

    // Draw and retain visible sprites.
    let write = 0
    for (let i = 0; i < sprites.length; i++) {
      const s = sprites[i]
      if (s.x + s.w < -40) continue
      context.fillText(s.text, s.x, s.y + 18)
      sprites[write++] = s
    }
    sprites.length = write

    // Show a lightweight hint when there's no data yet.
    if (sprites.length === 0 && ts - lastMsgAt > 1000) {
      context.save()
      context.shadowBlur = 0
      context.fillStyle = fg
      context.fillText('等待弹幕...（ESC 关闭）', 12, 24)
      context.restore()
    }

    // Occasional overload hint (best-effort): show 1 second at top-left.
    if (dropped > 0 && (ts | 0) % 1000 < 50) {
      context.save()
      context.shadowBlur = 0
      context.fillStyle = 'rgba(255,255,255,0.85)'
      context.fillText(`弹幕过快，已丢弃 ${dropped} 条`, 12, 20)
      context.restore()
      dropped = 0
    }

    raf = requestAnimationFrame(frame)
  }
  raf = requestAnimationFrame(frame)
  stopAnim = () => {
    stopped = true
    if (raf) cancelAnimationFrame(raf)
  }

  void (async () => {
    try {
      const un = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
        msgCount.value++
        enqueue(e.payload)
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
  stopAnim?.()
  stopResize?.()
  stopThemeWatch?.()
  stopKey?.()
  unMsg?.()
  if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
})

async function closeSelf() {
  if (!win) return
  try {
    await win.close()
  } catch {
    // ignore
  }
}

function startDrag(ev: PointerEvent) {
  if (ev.button !== 0) return
  // Drag undecorated window.
  if (!win) return
  void win.startDragging()
}
</script>

<template>
  <div :class="opaque ? 'overlay-root overlay-opaque' : 'overlay-root'">
    <div class="overlay-controls" @pointerdown="startDrag">
      <span class="overlay-title">Overlay ({{ msgCount }})</span>
      <span class="overlay-spacer" />
      <button class="overlay-btn" type="button" @click.stop="closeSelf">关闭</button>
    </div>
    <canvas ref="canvasEl" class="overlay-canvas" />
  </div>
</template>
