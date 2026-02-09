<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import type { DanmakuUiMessage } from '@/shared/types'

  type Sprite = {
    text: string
    x: number
    y: number
    w: number
    speedPxPerSec: number
  }

  const params = new URLSearchParams(window.location.search)
  const opaque = window.__CHAOS_SEED_BOOT?.overlayOpaque ?? params.get('overlay') === 'opaque'

  let canvasEl: HTMLCanvasElement | null = null
  let msgCount = 0
  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null
  let clickThrough = false

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
    // Don't start dragging when the user is clicking the close button.
    const t = ev.target as HTMLElement | null
    if (t && typeof t.closest === 'function' && t.closest('button')) return
    // Drag undecorated window.
    if (!win) return
    void win.startDragging()
  }

  onMount(() => {
    let disposed = false
    let unMsg: (() => void) | undefined
    let stopResize: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let onUnload: (() => void) | undefined

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    const cleanup = () => {
      disposed = true
      stopAnim?.()
      stopResize?.()
      stopKey?.()
      unMsg?.()
      if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
      void invoke('danmaku_set_msg_subscription', { enabled: false }).catch(() => {})
    }
    onUnload = () => cleanup()
    window.addEventListener('beforeunload', onUnload, true)

    if (!opaque) {
      // For transparent Tauri windows, the webview background must also be transparent.
      document.documentElement.style.background = 'transparent'
      document.body.style.background = 'transparent'
    }

    async function applyClickThrough(next: boolean) {
      if (!win) {
        clickThrough = false
        return
      }
      try {
        await win.setIgnoreCursorEvents(next)
        clickThrough = next
      } catch {
        // If the platform/webview doesn't support it (or permissions missing), keep the overlay usable.
        clickThrough = false
      }
    }

    // Default: interactive. Use F2 to toggle click-through on demand.
    void applyClickThrough(false)

    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === 'Escape') {
        void win?.close()
        return
      }
      if (ev.key === 'F2') {
        ev.preventDefault()
        void applyClickThrough(!clickThrough)
      }
    }
    window.addEventListener('keydown', onKey, true)
    stopKey = () => window.removeEventListener('keydown', onKey, true)

    if (!canvasEl) throw new Error('canvas not found')
    // TS doesn't reliably narrow captured variables inside nested functions; capture a non-null handle.
    const canvas2: HTMLCanvasElement = canvasEl

    const ctx = canvas2.getContext('2d')
    if (!ctx) throw new Error('canvas 2d context not available')
    const context: CanvasRenderingContext2D = ctx

    // Visual rule:
    // - Transparent overlay: black text
    // - Opaque overlay: white text
    const fg = opaque ? '#ffffff' : '#000000'
    const shadow = opaque ? 'rgba(0,0,0,0.75)' : 'rgba(255,255,255,0.85)'
    const bg = '#0b1220'

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
      context.shadowColor = shadow
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
        const y = 24
        context.fillText('等待弹幕...（ESC 关闭）', 12, y)
        if (clickThrough) {
          context.fillText('鼠标穿透已开启：Alt+Tab 聚焦后按 F2 切换交互', 12, y + 20)
        }
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

      // Interaction hint: when click-through is enabled, the overlay can't be clicked.
      if (clickThrough) {
        context.save()
        context.shadowBlur = 0
        context.globalAlpha = 0.7
        context.fillStyle = fg
        const y = Math.max(18, h - 14)
        context.fillText('鼠标穿透已开启：Alt+Tab 聚焦后按 F2 切换交互，ESC 关闭', 12, y)
        context.restore()
      }

      // Visual resize hint: a thick border so the user can see the window edges clearly.
      context.save()
      context.shadowBlur = 0
      context.globalAlpha = 0.35
      context.strokeStyle = opaque ? 'rgba(255,255,255,0.8)' : 'rgba(0,0,0,0.8)'
      context.lineWidth = 6
      context.strokeRect(3, 3, Math.max(1, w - 6), Math.max(1, h - 6))
      context.restore()

      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)
    stopAnim = () => {
      stopped = true
      if (raf) cancelAnimationFrame(raf)
    }

    void (async () => {
      try {
        // Overlay is a renderer: subscribe to high-frequency danmaku messages while it is open.
        void invoke('danmaku_set_msg_subscription', { enabled: true }).catch(() => {})

        const un = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
          msgCount++
          enqueue(e.payload)
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

<div class={opaque ? 'overlay-root overlay-opaque' : 'overlay-root'}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="overlay-controls" on:pointerdown={startDrag}>
    <span class="overlay-title">Overlay ({msgCount})</span>
    <span class="overlay-spacer"></span>
    <button class="overlay-btn" type="button" on:click|stopPropagation={closeSelf}>关闭</button>
  </div>
  <canvas bind:this={canvasEl} class="overlay-canvas"></canvas>
</div>
