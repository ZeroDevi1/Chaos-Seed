import { invoke } from '@tauri-apps/api/core'

import type { DanmakuUiMessage } from '../shared/types'

type DanmakuImageReply = {
  mime: string
  bytes: number[]
}

const TRANSPARENT_PIXEL =
  'data:image/gif;base64,R0lGODlhAQABAAAAACwAAAAAAQABAAA='

type CachedImage = {
  url: string
  objectUrl: string
  lastUsedAt: number
}

const imageCache = new Map<string, CachedImage>()
const inflight = new Map<string, Promise<string>>()
const MAX_CACHED_IMAGES = 200

function touchCache(key: string) {
  const v = imageCache.get(key)
  if (v) v.lastUsedAt = Date.now()
}

function evictIfNeeded() {
  if (imageCache.size <= MAX_CACHED_IMAGES) return
  const entries = [...imageCache.entries()]
  entries.sort((a, b) => a[1].lastUsedAt - b[1].lastUsedAt)
  const toEvict = entries.slice(0, Math.max(10, Math.floor(entries.length / 5)))
  for (const [k, v] of toEvict) {
    imageCache.delete(k)
    try {
      URL.revokeObjectURL(v.objectUrl)
    } catch {
      // ignore
    }
  }
}

async function fetchImageObjectUrl(msg: DanmakuUiMessage, url: string): Promise<string> {
  const key = url
  const cached = imageCache.get(key)
  if (cached) {
    touchCache(key)
    return cached.objectUrl
  }

  const existing = inflight.get(key)
  if (existing) return existing

  const p = (async () => {
    const reply = await invoke<DanmakuImageReply>('danmaku_fetch_image', {
      url,
      site: msg.site,
      room_id: msg.room_id
    })
    const mime = reply.mime?.trim() || 'image/png'
    const buf = new Uint8Array(reply.bytes)
    const blob = new Blob([buf], { type: mime })
    const objectUrl = URL.createObjectURL(blob)
    imageCache.set(key, { url, objectUrl, lastUsedAt: Date.now() })
    inflight.delete(key)
    evictIfNeeded()
    return objectUrl
  })().catch((e) => {
    inflight.delete(key)
    throw e
  })

  inflight.set(key, p)
  return p
}

export type DanmakuListStore = {
  enqueue(msg: DanmakuUiMessage): void
  clear(): void
  dispose(): void
}

type Opts = {
  scrollEl: HTMLElement
  listEl: HTMLElement
  statusEl?: HTMLElement
  maxItems?: number
  flushIntervalMs?: number
  stickToBottom?: boolean
  onOpenUrl?: (url: string) => void
  afterFlush?: (listCount: number) => void
}

function renderRow(msg: DanmakuUiMessage, onOpenUrl?: (url: string) => void): HTMLElement {
  const row = document.createElement('div')
  row.className = 'dm-row'

  const user = document.createElement('span')
  user.className = 'dm-user'
  user.textContent = `${msg.user || msg.site}：`
  row.appendChild(user)

  const imageUrl = msg.image_url ?? undefined
  if (imageUrl) {
    const img = document.createElement('img')
    img.className = 'dm-image'
    img.src = TRANSPARENT_PIXEL
    img.alt = ''
    img.loading = 'lazy'
    img.decoding = 'async'
    if (msg.image_width && Number.isFinite(msg.image_width)) {
      img.style.width = `${Math.max(18, Math.min(96, msg.image_width))}px`
    }
    if (onOpenUrl) {
      img.style.cursor = 'pointer'
      img.onclick = () => onOpenUrl(imageUrl)
    }

    // Try to proxy-load images to avoid hotlink/referrer restrictions (common on live platforms).
    // If it fails, fall back to direct URL.
    void fetchImageObjectUrl(msg, imageUrl)
      .then((objectUrl) => {
        if (!img.isConnected) return
        img.src = objectUrl
      })
      .catch(() => {
        if (!img.isConnected) return
        img.src = imageUrl
      })

    img.onerror = () => {
      // Hide broken thumbnails (avoid repeated "404" icons).
      img.remove()
    }

    row.appendChild(img)
  }

  const shownText =
    msg.image_url && (msg.text === '[图片]' || msg.text === '[表情]') ? '' : (msg.text || '')
  const text = document.createElement('span')
  text.className = 'dm-text'
  text.textContent = shownText
  row.appendChild(text)

  return row
}

export function createDanmakuListStore(opts: Opts): DanmakuListStore {
  const maxItems = opts.maxItems ?? 400
  // Default to frame-batched rendering for low latency (no visible "timer refresh" feel).
  // Tests can still force interval flushing via `flushIntervalMs`.
  const flushIntervalMs = opts.flushIntervalMs ?? 0
  const maxBatch = 80
  const dedupeWindowMs = 80

  let queue: DanmakuUiMessage[] = []
  let dropped = 0
  let disposed = false
  const recent = new Map<string, number>()
  const useInterval = Number.isFinite(flushIntervalMs) && flushIntervalMs > 0
  let scheduledMicro = false
  let scheduledRaf = false
  let raf = 0

  function enqueue(msg: DanmakuUiMessage) {
    if (disposed) return

    // Defensive dedupe: avoid rendering the same payload multiple times if the backend/webview
    // delivers duplicates very close together.
    const key = `${msg.user}|${msg.text}|${msg.image_url ?? ''}|${msg.image_width ?? ''}`
    const now = Date.now()
    const last = recent.get(key)
    if (last !== undefined && now - last < dedupeWindowMs) return
    recent.set(key, now)
    if (recent.size > 600) {
      // Drop old entries.
      for (const [k, t] of recent) {
        if (now - t > 5000) recent.delete(k)
      }
    }

    queue.push(msg)

    // Overload protection: keep the newest part of the queue.
    if (queue.length > 1000) {
      const keep = queue.slice(-200)
      dropped += queue.length - keep.length
      queue = keep
    }

    if (!useInterval) scheduleFlushMicro()
  }

  function scheduleFlushMicro() {
    if (disposed) return
    if (scheduledMicro) return
    scheduledMicro = true
    // Render ASAP after a Rust push (no perceptible timer delay).
    // If we still have backlog after the microtask flush, continue draining over animation frames
    // to avoid starving input/painter.
    queueMicrotask(() => {
      scheduledMicro = false
      flush()
      if (queue.length > 0) scheduleFlushRaf()
    })
  }

  function scheduleFlushRaf() {
    if (disposed) return
    if (scheduledRaf) return
    scheduledRaf = true
    raf = requestAnimationFrame(() => {
      scheduledRaf = false
      flush()
      if (queue.length > 0) scheduleFlushRaf()
    })
  }

  function flush() {
    if (disposed) return
    if (queue.length === 0) return

    const atBottom =
      opts.scrollEl.scrollHeight - (opts.scrollEl.scrollTop + opts.scrollEl.clientHeight) < 64

    const batch = queue.splice(0, Math.min(queue.length, maxBatch))

    const frag = document.createDocumentFragment()
    for (const msg of batch) frag.appendChild(renderRow(msg, opts.onOpenUrl))
    opts.listEl.appendChild(frag)

    while (opts.listEl.childElementCount > maxItems) {
      opts.listEl.removeChild(opts.listEl.firstElementChild!)
    }

    if (dropped > 0 && opts.statusEl) {
      opts.statusEl.textContent = `弹幕过快，已丢弃 ${dropped} 条（为保证流畅性）。`
      dropped = 0
    }

    // For chat-like views we always keep the newest messages in view.
    if (opts.stickToBottom || atBottom) opts.scrollEl.scrollTop = opts.scrollEl.scrollHeight
    opts.afterFlush?.(opts.listEl.childElementCount)
  }

  const timer = useInterval ? window.setInterval(flush, flushIntervalMs) : 0

  return {
    enqueue,
    clear: () => {
      queue = []
      dropped = 0
      opts.listEl.innerHTML = ''
      opts.afterFlush?.(0)
    },
    dispose: () => {
      disposed = true
      if (timer) window.clearInterval(timer)
      if (raf) cancelAnimationFrame(raf)
    }
  }
}
