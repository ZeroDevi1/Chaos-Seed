<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import { onMount } from 'svelte'

  import MasterPlayer from '@/player/MasterPlayer.svelte'
  import type { PlayerBootRequest } from '@/player/types'
  import type { StreamVariant } from '@/shared/livestreamTypes'

  let player: MasterPlayer | null = null
  let bootReq: PlayerBootRequest | null = null
  let lastErr = ''

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null

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
    try {
      await player?.load(req)
    } catch (e) {
      lastErr = String(e)
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
    let disposed = false
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
      void applyReq(maybe as PlayerBootRequest)
    }

    // Subsequent loads triggered by main window.
    void (async () => {
      try {
        const un = await listen<PlayerBootRequest>('player_load', (e) => {
          if (disposed) return
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

    return cleanup
  })
</script>

<div class="player-window">
  <MasterPlayer bind:this={player} boot={bootReq} />
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
