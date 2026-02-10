<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import { fetchDanmakuImage } from '@/shared/danmakuApi'
  import type { DanmakuUiMessage } from '@/shared/types'

  type Sprite = {
    text: string
    img?: HTMLImageElement
    imgW?: number
    imgH?: number
    x: number
    y: number
    w: number
    speedPxPerSec: number
    laneIndex: number
  }

  type QueueItem = {
    text: string
    img?: HTMLImageElement
    imgW?: number
    imgH?: number
  }

  const params = new URLSearchParams(window.location.search)
  const opaque = window.__CHAOS_SEED_BOOT?.overlayOpaque ?? params.get('overlay') === 'opaque'

  let canvasEl: HTMLCanvasElement | null = null
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

  onMount(() => {
    let disposed = false
    let unMsg: (() => void) | undefined
    let stopResize: (() => void) | undefined
    let stopAnim: (() => void) | undefined
    let stopKey: (() => void) | undefined
    let onUnload: (() => void) | undefined
    const objectUrls: string[] = []

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
      for (const u of objectUrls) {
        try {
          URL.revokeObjectURL(u)
        } catch {
          // ignore
        }
      }
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
    // - Transparent overlay: WHITE text (per requirement), avoid blur shadows that look like "double".
    //   Use a crisp outline instead for readability on bright backgrounds.
    // - Opaque overlay: white text, no heavy shadow (avoid duplicate-looking glyphs).
    const fg = '#ffffff'
    // Opaque overlay uses a stable dark background regardless of theme (better readability).
    const bg = '#1f1f1f'

    const sprites: Sprite[] = []
    const laneHeight = 32
    const topPad = 12
    const bottomPad = 32

    function computeLaneCount(height: number): number {
      const usable = Math.max(0, Math.floor(height - topPad - bottomPad))
      return Math.max(1, Math.floor(usable / laneHeight))
    }

    let laneCursor = 0
    let laneCount = 1

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

      laneCount = computeLaneCount(h)
      // Reflow existing sprites without dropping them (resize should not interrupt the stream).
      const yMax = Math.max(topPad, h - bottomPad - laneHeight)
      for (let i = 0; i < sprites.length; i++) {
        const s = sprites[i]
        s.laneIndex = s.laneIndex % laneCount
        s.y = topPad + s.laneIndex * laneHeight
        s.y = Math.min(s.y, yMax)
      }
    }
    resize()
    window.addEventListener('resize', resize)
    stopResize = () => window.removeEventListener('resize', resize)

    let queue: QueueItem[] = []
    let qHead = 0
    let dropped = 0
    let lastMsgAt = 0
    const dedupeWindowMs = 80
    const recent = new Map<string, number>()

    const imgCache = new Map<string, { img: HTMLImageElement; objectUrl?: string; lastUsedAt: number }>()
    const imgInflight = new Map<string, Promise<HTMLImageElement>>()
    const MAX_CACHED_IMAGES = 120

    function evictImagesIfNeeded() {
      if (imgCache.size <= MAX_CACHED_IMAGES) return
      const entries = [...imgCache.entries()]
      entries.sort((a, b) => a[1].lastUsedAt - b[1].lastUsedAt)
      const toEvict = entries.slice(0, Math.max(10, Math.floor(entries.length / 4)))
      for (const [key, v] of toEvict) {
        imgCache.delete(key)
        if (v.objectUrl) {
          try {
            URL.revokeObjectURL(v.objectUrl)
          } catch {
            // ignore
          }
        }
      }
    }

    async function loadImage(msg: DanmakuUiMessage, url: string): Promise<HTMLImageElement> {
      const cached = imgCache.get(url)
      if (cached) {
        cached.lastUsedAt = Date.now()
        return cached.img
      }
      const inflight = imgInflight.get(url)
      if (inflight) return inflight

      const p = (async () => {
        // Prefer proxy-loading via Rust to avoid hotlink/referrer issues.
        let objectUrl: string | undefined
        try {
          const reply = await fetchDanmakuImage({ url, site: msg.site, roomId: msg.room_id })
          const mime = reply.mime?.trim() || 'image/png'
          const buf = new Uint8Array(reply.bytes)
          const blob = new Blob([buf], { type: mime })
          objectUrl = URL.createObjectURL(blob)
          objectUrls.push(objectUrl)
        } catch {
          // Fallback: try direct URL (may still work for some platforms).
          objectUrl = undefined
        }

        const img = new Image()
        img.decoding = 'async'
        img.loading = 'eager'
        img.src = objectUrl ?? url
        // Wait for load/decode so we can measure dimensions reliably.
        await (img.decode?.() ?? new Promise<void>((resolve, reject) => {
          img.onload = () => resolve()
          img.onerror = () => reject(new Error('image load failed'))
        }))

        imgCache.set(url, { img, objectUrl, lastUsedAt: Date.now() })
        imgInflight.delete(url)
        evictImagesIfNeeded()
        return img
      })().catch((e) => {
        imgInflight.delete(url)
        throw e
      })

      imgInflight.set(url, p)
      return p
    }

    function pushQueue(item: QueueItem) {
      queue.push(item)
      lastMsgAt = performance.now()
      if (queue.length > 600) {
        const keep = queue.slice(-120)
        dropped += queue.length - keep.length
        queue = keep
        qHead = 0
      }
    }

    function enqueue(msg: DanmakuUiMessage) {
      const key = `${msg.user}|${msg.text}|${msg.image_url ?? ''}|${msg.image_width ?? ''}`
      const now = performance.now()
      const last = recent.get(key)
      if (last !== undefined && now - last < dedupeWindowMs) return
      recent.set(key, now)
      if (recent.size > 800) {
        for (const [k, t] of recent) {
          if (now - t > 5000) recent.delete(k)
        }
      }

      const imageUrl = msg.image_url ?? undefined
      const rawText = (msg.text || '').toString()
      const text =
        imageUrl && (rawText === '[图片]' || rawText === '[表情]') ? '' : rawText.trim()

      if (imageUrl) {
        void loadImage(msg, imageUrl)
          .then((img) => {
            if (disposed) return
            const targetH = 24
            const ratio = img.naturalHeight > 0 ? img.naturalWidth / img.naturalHeight : 1
            const imgH = targetH
            const imgW = Math.max(18, Math.min(96, Math.round(targetH * ratio)))
            pushQueue({ text, img, imgW, imgH })
          })
          .catch(() => {
            // If the image fails to load, fall back to text so the message isn't lost.
            if (disposed) return
            pushQueue({ text: text || '[表情]' })
          })
        return
      }

      pushQueue({ text })
    }

    function spawn(maxPerFrame: number) {
      const pending = queue.length - qHead
      if (pending <= 0) return
      const n = Math.min(maxPerFrame, pending)
      for (let i = 0; i < n; i++) {
        const item = queue[qHead++]!
        const text = (item.text || '').trim()
        if (!text && !item.img) continue

        context.font = '18px system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif'
        const textW = text ? Math.ceil(context.measureText(text).width) : 0
        const imgW = item.img ? (item.imgW ?? 24) : 0
        const gap = item.img && text ? 8 : 0
        const tw = imgW + gap + textW

        const laneIndex = laneCursor % laneCount
        laneCursor++
        const y = topPad + laneIndex * laneHeight
        const durationMs = 8000
        const distance = w + tw + 80
        const speedPxPerSec = distance / (durationMs / 1000)
        sprites.push({
          text,
          img: item.img,
          imgW: item.imgW,
          imgH: item.imgH,
          x: w + 40,
          y,
          w: tw,
          speedPxPerSec,
          laneIndex
        })
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

      // Spawn more per frame to reduce perceived latency (avoid "timer refresh" feel).
      spawn(6)

      if (opaque) {
        context.fillStyle = bg
        context.fillRect(0, 0, w, h)
      } else {
        context.clearRect(0, 0, w, h)
      }

      context.font = '18px system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif'
      context.fillStyle = fg
      context.shadowBlur = 0
      if (!opaque) {
        context.lineJoin = 'round'
        context.miterLimit = 2
        context.lineWidth = 3
        context.strokeStyle = 'rgba(0,0,0,0.65)'
      }
      for (let i = 0; i < sprites.length; i++) {
        sprites[i].x -= sprites[i].speedPxPerSec * (dt / 1000)
      }

      // Draw and retain visible sprites.
      let write = 0
      for (let i = 0; i < sprites.length; i++) {
        const s = sprites[i]
        if (s.x + s.w < -40) continue
        if (!opaque) {
          // Crisp outline helps on transparent overlays without looking like blurred double text.
          if (s.img) {
            // no outline needed for the image
          }
          if (s.text) context.strokeText(s.text, s.x + (s.img ? (s.imgW ?? 24) + (s.text ? 8 : 0) : 0), s.y + 18)
        }
        if (s.img) {
          const ih = s.imgH ?? 24
          const iw = s.imgW ?? 24
          const iy = s.y + 18 - ih + 4
          try {
            context.drawImage(s.img, s.x, iy, iw, ih)
          } catch {
            // ignore draw errors
          }
        }
        if (s.text) {
          const tx = s.x + (s.img ? (s.imgW ?? 24) + (s.text ? 8 : 0) : 0)
          context.fillText(s.text, tx, s.y + 18)
        }
        sprites[write++] = s
      }
      sprites.length = write

      // Show a lightweight hint when there's no data yet.
      if (sprites.length === 0 && ts - lastMsgAt > 1000) {
        context.save()
        context.shadowBlur = 0
        context.fillStyle = fg
        const y = 24
        if (!opaque) context.strokeText('等待弹幕...（ESC 关闭）', 12, y)
        context.fillText('等待弹幕...（ESC 关闭）', 12, y)
        if (clickThrough) {
          const hint = '鼠标穿透已开启：Alt+Tab 聚焦后按 F2 切换交互'
          if (!opaque) context.strokeText(hint, 12, y + 20)
          context.fillText(hint, 12, y + 20)
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
        const hint = '鼠标穿透已开启：Alt+Tab 聚焦后按 F2 切换交互，ESC 关闭'
        if (!opaque) context.strokeText(hint, 12, y)
        context.fillText(hint, 12, y)
        context.restore()
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
        // Overlay is a renderer: subscribe to high-frequency danmaku messages while it is open.
        void invoke('danmaku_set_msg_subscription', { enabled: true }).catch(() => {})

        const un = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
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
  <canvas bind:this={canvasEl} class="overlay-canvas"></canvas>
</div>
