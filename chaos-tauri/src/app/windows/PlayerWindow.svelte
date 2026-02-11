<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onDestroy, onMount } from 'svelte'
  import { fade } from 'svelte/transition'

  import MasterPlayer from '@/player/MasterPlayer.svelte'
  import type { PlayerBootRequest } from '@/player/types'
  import { fetchDanmakuImage } from '@/shared/danmakuApi'
  import type { StreamVariant } from '@/shared/livestreamTypes'

  let player: MasterPlayer | null = null
  let bootReq: PlayerBootRequest | null = null
  let lastErr = ''

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null
  let disposed = false
  let heroMinMs = 0
  let heroMinDone = true
  let ready = false
  let prepared = false
  let preparingKey = ''

  let posterObjectUrl: string | null = null
  let posterKey = ''
  let posterFadeMs = 180

  function revokePosterObjectUrl() {
    if (!posterObjectUrl) return
    try {
      URL.revokeObjectURL(posterObjectUrl)
    } catch {
      // ignore
    }
    posterObjectUrl = null
  }

  onDestroy(() => {
    revokePosterObjectUrl()
  })

  async function closeSelf() {
    if (!win) return
    try {
      await win.close()
    } catch {
      // ignore
    }
  }

  async function applyReq(req: PlayerBootRequest) {
    bootReq = req
    lastErr = ''
    ready = false
    prepared = false
    const k = `${req.site}:${req.room_id}:${req.variant_id}:${req.url}`
    preparingKey = k
    try {
      await player?.prepare(req)
      if (disposed || preparingKey !== k) return
      prepared = true
      await maybeStart()
    } catch (e) {
      lastErr = String(e)
      ready = false
      prepared = false
    }
  }

  async function maybeStart() {
    if (disposed) return
    if (!heroMinDone) return
    if (!prepared) return
    if (ready) return
    try {
      await player?.start()
      if (disposed) return
      ready = true
    } catch (e) {
      lastErr = String(e)
      ready = false
    }
  }

  async function syncPoster(req: PlayerBootRequest | null) {
    if (!req) {
      posterKey = ''
      revokePosterObjectUrl()
      return
    }

    const raw = (req.cover ?? '').toString().trim()
    const nextKey = `${req.site}|${req.room_id}|${raw}`
    if (nextKey === posterKey) return
    posterKey = nextKey
    const myKey = nextKey

    revokePosterObjectUrl()
    if (!raw) return

    try {
      const reply = await fetchDanmakuImage({ url: raw, site: req.site, roomId: req.room_id })
      const mime = (reply.mime || '').toString().trim() || 'image/jpeg'
      const buf = new Uint8Array(reply.bytes)
      const blob = new Blob([buf], { type: mime })
      const objectUrl = URL.createObjectURL(blob)
      if (posterKey !== myKey) {
        try {
          URL.revokeObjectURL(objectUrl)
        } catch {
          // ignore
        }
        return
      }
      posterObjectUrl = objectUrl
    } catch {
      posterObjectUrl = null
    }
  }

  function getVariants(req: PlayerBootRequest | null): StreamVariant[] {
    const v = (req?.variants ?? null) as any
    return Array.isArray(v) ? (v as StreamVariant[]) : []
  }

  async function onVariantPick(ev: Event) {
    if (!bootReq) return
    const nextId = ((ev.target as unknown as { value: string })?.value ?? '').toString()
    if (!nextId) return
    const vars = getVariants(bootReq)
    const v = vars.find((x) => x.id === nextId)
    if (!v) return

    let url = (v.url ?? '').toString().trim()
    let backup_urls = Array.isArray(v.backup_urls) ? v.backup_urls : []
    if (!url) {
      try {
        const resolved = await invoke<StreamVariant>('livestream_resolve_variant', {
          site: bootReq.site,
          roomId: bootReq.room_id,
          variantId: v.id
        })
        url = (resolved.url ?? '').toString().trim()
        backup_urls = resolved.backup_urls ?? []
        // Update the cached variants list so subsequent switches don't re-resolve.
        const merged = vars.map((x) => (x.id === resolved.id ? { ...x, ...resolved } : x))
        bootReq = { ...bootReq, variants: merged }
      } catch (e) {
        lastErr = `解析线路失败：${String(e)}`
        return
      }
    }

    if (!url) {
      lastErr = '该线路暂无直连 URL'
      return
    }

    await applyReq({
      ...bootReq,
      variant_id: v.id,
      variant_label: v.label,
      url,
      backup_urls
    })
  }

  onMount(() => {
    let unLoad: (() => void) | undefined
    let onKey: ((ev: KeyboardEvent) => void) | undefined
    let onUnload: (() => void) | undefined

    try {
      win = getCurrentWebviewWindow()
    } catch {
      win = null
    }

    const cleanup = () => {
      disposed = true
      unLoad?.()
      if (onKey) window.removeEventListener('keydown', onKey, true)
      if (onUnload) window.removeEventListener('beforeunload', onUnload, true)
      void player?.destroy()
    }

    onUnload = () => cleanup()
    window.addEventListener('beforeunload', onUnload, true)

    onKey = (ev) => {
      if (ev.key === 'Escape') void closeSelf()
    }
    window.addEventListener('keydown', onKey, true)

    // Initial load from boot params (production-safe).
    const boot = window.__CHAOS_SEED_BOOT as any
    const maybe = boot?.player
    if (maybe && typeof maybe === 'object') {
      const delay = Number(boot?.heroDelayMs ?? 0)
      heroMinMs = Number.isFinite(delay) ? Math.max(0, Math.round(delay)) : 0
      heroMinDone = heroMinMs <= 0
      if (!heroMinDone) {
        window.setTimeout(() => {
          if (disposed) return
          heroMinDone = true
          void maybeStart()
        }, heroMinMs)
      }

      // Start loading immediately; we keep a poster overlay for `heroMinMs` to avoid visual pop.
      void applyReq(maybe as PlayerBootRequest)
    }

    // Subsequent loads triggered by main window.
    void (async () => {
      try {
        const un = await listen<PlayerBootRequest>('player_load', (e) => {
          if (disposed) return
          heroMinMs = 0
          heroMinDone = true
          void applyReq(e.payload)
        })
        if (disposed) return un()
        unLoad = un
      } catch {
        // ignore
      }
    })()

    // Best-effort: keep the same Win11 backdrop mode as main/chat when player opens.
    void invoke('set_backdrop', { mode: 'none' }).catch(() => {})

    try {
      posterFadeMs = window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches ? 0 : 180
    } catch {
      posterFadeMs = 180
    }

    return cleanup
  })

  $: void syncPoster(bootReq)
</script>

<div class="player-window">
  <MasterPlayer bind:this={player} boot={bootReq} />
  {#if bootReq && !lastErr && (posterObjectUrl || (bootReq.cover ?? '').toString().trim()) && !ready}
    <div class="player-window-poster" aria-hidden="true" out:fade={{ duration: posterFadeMs }}>
      {#if posterObjectUrl}
        <img class="player-window-poster-img" alt="" src={posterObjectUrl} />
      {:else}
        <img class="player-window-poster-img" alt="" src={bootReq.cover} />
      {/if}
      <div class="player-window-poster-shade"></div>
      <div class="player-window-poster-meta">
        <div class="player-window-poster-title">{bootReq.title}</div>
        <div class="player-window-poster-sub">{bootReq.site} / {bootReq.room_id} / {bootReq.variant_label}</div>
      </div>
    </div>
  {/if}
  {#if bootReq && getVariants(bootReq).length > 1}
    <div class="player-window-top">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <select class="player-window-select" value={bootReq.variant_id} on:change={onVariantPick}>
        {#each getVariants(bootReq) as v (v.id)}
          <option value={v.id}>{v.label}</option>
        {/each}
      </select>
      <button class="player-window-btn" on:click={closeSelf}>关闭</button>
    </div>
  {/if}
  {#if lastErr}
    <div class="player-window-err">{lastErr}</div>
  {/if}
</div>

<style>
  .player-window {
    position: relative;
    width: 100%;
    height: 100%;
    background: #000;
  }

  .player-window-poster {
    position: absolute;
    inset: 0;
    z-index: 5;
    overflow: hidden;
    pointer-events: none;
    background: #000;
  }

  .player-window-poster-img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scale(1.02);
    filter: blur(10px) saturate(1.05);
    opacity: 0.92;
  }

  .player-window-poster-shade {
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 30% 20%, rgba(0, 0, 0, 0.15), rgba(0, 0, 0, 0.72));
  }

  .player-window-poster-meta {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    color: #fff;
  }

  .player-window-poster-title {
    font-weight: 700;
    font-size: 16px;
    line-height: 1.25;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  }

  .player-window-poster-sub {
    margin-top: 4px;
    opacity: 0.78;
    font-size: 12px;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
  }

  .player-window-top {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    gap: 10px;
    align-items: center;
    z-index: 10;
  }

  .player-window-select {
    height: 30px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
    padding: 0 10px;
  }

  .player-window-btn {
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    cursor: pointer;
  }

  .player-window-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .player-window-err {
    position: absolute;
    left: 12px;
    right: 12px;
    top: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(255, 0, 0, 0.18);
    border: 1px solid rgba(255, 0, 0, 0.35);
    color: #fff;
    font-size: 12px;
    white-space: pre-wrap;
    pointer-events: none;
  }
</style>
