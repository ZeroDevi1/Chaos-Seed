<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'

  import { createEngine } from './engineFactory'
  import type { PlayerBootRequest, PlayerEngine, PlayerEngineKind, PlayerSource } from './types'
  import { choosePauseStrategy } from './playbackPolicy'
  import { expandHttpToHttps } from './urlNormalize'
  import { inferStreamType } from './utils'

  export let boot: PlayerBootRequest | null = null

  let rootEl: HTMLDivElement | null = null
  let stageEl: HTMLDivElement | null = null

  let engine: PlayerEngine | null = null
  let engineKind: PlayerEngineKind = 'native'

  let playing = false
  let muted = false
  let volume = 100 // 0..100

  let showControls = true
  let hideTimer: number | null = null

  let currentKey = ''
  let lastErr = ''
  let lastReq: PlayerBootRequest | null = null
  let pausedByStop = false
  let prepared = false

  function clearHideTimer() {
    if (hideTimer) {
      window.clearTimeout(hideTimer)
      hideTimer = null
    }
  }

  function scheduleHide() {
    clearHideTimer()
    if (!playing) return
    hideTimer = window.setTimeout(() => {
      showControls = false
    }, 2000)
  }

  function bumpControls() {
    showControls = true
    scheduleHide()
  }

  async function ensureEngine(kind: PlayerEngineKind) {
    if (!stageEl) throw new Error('stage not ready')
    if (engine && engineKind === kind) return
    if (engine) {
      try {
        await engine.destroy()
      } catch {
        // ignore
      }
    }
    engineKind = kind
    engine = createEngine(kind)
    await engine.init(stageEl)
    engine.setMuted(muted)
    engine.setVolume(volume / 100)
  }

  async function recreateEngine(kind: PlayerEngineKind) {
    if (engine) {
      try {
        await engine.destroy()
      } catch {
        // ignore
      }
    }
    engine = null
    await ensureEngine(kind)
  }

  async function loadInternal(req: PlayerBootRequest) {
    lastReq = req
    prepared = false
    lastErr = ''
    const primary = (req?.url || '').toString().trim()
    const backups = (req?.backup_urls || []).map((u) => (u || '').toString().trim()).filter(Boolean)
    const urlsRaw = [primary, ...backups].filter(Boolean)
    if (urlsRaw.length === 0) throw new Error('empty url')

    // Field feedback suggests some BiliLive primary hosts can fail in embedded webviews.
    // Prefer backups first for BiliLive; still keep a fallback chain for other sites too.
    const candidates = (req.site === 'bili_live' ? [...backups, primary] : urlsRaw)
      .flatMap((u) => expandHttpToHttps(u))
      .map((u) => u.trim())
      .filter(Boolean)
      .filter((u, idx, arr) => arr.indexOf(u) === idx) // de-dupe while preserving order

    let last: unknown = null
    for (const url of candidates) {
      const kindFromUrl = inferStreamType(url)
      const kind: PlayerEngineKind =
        kindFromUrl === 'hls' ? 'hls' : kindFromUrl === 'flv' ? 'avplayer' : 'native'
      try {
        // Recreate engine between attempts to ensure failed network/decoder state is fully torn down.
        await recreateEngine(kind)

        const source: PlayerSource = {
          url,
          backup_urls: candidates.filter((x) => x !== url),
          isLive: true,
          kind,
          referer: req.referer,
          user_agent: req.user_agent
        }
        await engine!.load(source)
        await engine!.play()
        playing = true
        prepared = true
        scheduleHide()
        lastErr = ''
        break
      } catch (e) {
        last = e
        lastErr = `尝试播放失败：${String(e)}`
        // continue to next backup
      }
    }
    if (!playing) {
      throw last instanceof Error ? last : new Error(String(last ?? 'play failed'))
    }

    // Some engines use an underlying <video>; attach best-effort listeners to keep UI state in sync.
    await tick()
    const video = stageEl?.querySelector('video') as HTMLVideoElement | null
    if (video) {
      const onPlay = () => {
        playing = true
        scheduleHide()
      }
      const onPause = () => {
        playing = false
        showControls = true
        clearHideTimer()
      }
      video.addEventListener('play', onPlay, true)
      video.addEventListener('pause', onPause, true)
      const off = () => {
        video.removeEventListener('play', onPlay, true)
        video.removeEventListener('pause', onPause, true)
      }
      cleanupFns.push(off)
    }
  }

  async function prepareInternal(req: PlayerBootRequest) {
    lastReq = req
    prepared = false
    lastErr = ''
    const primary = (req?.url || '').toString().trim()
    const backups = (req?.backup_urls || []).map((u) => (u || '').toString().trim()).filter(Boolean)
    const urlsRaw = [primary, ...backups].filter(Boolean)
    if (urlsRaw.length === 0) throw new Error('empty url')

    const candidates = (req.site === 'bili_live' ? [...backups, primary] : urlsRaw)
      .flatMap((u) => expandHttpToHttps(u))
      .map((u) => u.trim())
      .filter(Boolean)
      .filter((u, idx, arr) => arr.indexOf(u) === idx)

    let last: unknown = null
    for (const url of candidates) {
      const kindFromUrl = inferStreamType(url)
      const kind: PlayerEngineKind =
        kindFromUrl === 'hls' ? 'hls' : kindFromUrl === 'flv' ? 'avplayer' : 'native'
      try {
        await recreateEngine(kind)
        const source: PlayerSource = {
          url,
          backup_urls: candidates.filter((x) => x !== url),
          isLive: true,
          kind,
          referer: req.referer,
          user_agent: req.user_agent
        }
        await engine!.load(source)
        prepared = true
        lastErr = ''
        break
      } catch (e) {
        last = e
        lastErr = `尝试预加载失败：${String(e)}`
      }
    }
    if (!prepared) {
      throw last instanceof Error ? last : new Error(String(last ?? 'prepare failed'))
    }
  }

  const cleanupFns: Array<() => void> = []
  async function destroyInternal() {
    clearHideTimer()
    while (cleanupFns.length > 0) {
      try {
        cleanupFns.pop()?.()
      } catch {
        // ignore
      }
    }
    if (engine) {
      try {
        await engine.destroy()
      } catch {
        // ignore
      }
    }
    engine = null
    playing = false
    prepared = false
  }

  export async function load(req: PlayerBootRequest) {
    const nextKey = `${req.site}:${req.room_id}:${req.variant_id}:${req.url}`
    if (nextKey === currentKey) return
    currentKey = nextKey
    pausedByStop = false
    await destroyInternal()
    await loadInternal(req)
  }

  export async function prepare(req: PlayerBootRequest) {
    const nextKey = `${req.site}:${req.room_id}:${req.variant_id}:${req.url}`
    if (nextKey === currentKey && prepared && !playing) return
    currentKey = nextKey
    pausedByStop = false
    await destroyInternal()
    await prepareInternal(req)
  }

  export async function start() {
    if (!engine) throw new Error('engine not ready')
    if (playing) return
    await engine.play()
    playing = true
    prepared = true
    scheduleHide()
  }

  export async function destroy() {
    pausedByStop = false
    await destroyInternal()
  }

  async function togglePlay() {
    try {
      if (playing) {
        const strat = choosePauseStrategy({ engineKind, isLive: true })
        if (strat === 'stop') {
          pausedByStop = true
          await destroyInternal()
          showControls = true
        } else {
          if (!engine) return
          await engine.pause()
          playing = false
          showControls = true
          clearHideTimer()
        }
      } else {
        if (pausedByStop && lastReq) {
          pausedByStop = false
          await loadInternal(lastReq)
        } else {
          if (!engine) return
          await engine.play()
          playing = true
          scheduleHide()
        }
      }
    } catch (e) {
      lastErr = `播放控制失败：${String(e)}`
    }
  }

  function toggleMute() {
    muted = !muted
    engine?.setMuted(muted)
  }

  function onVolumeInput(ev: Event) {
    const v = Number(((ev.target as unknown as { value: string })?.value ?? '100').toString())
    volume = Number.isFinite(v) ? Math.max(0, Math.min(100, Math.round(v))) : 100
    engine?.setVolume(volume / 100)
  }

  async function toggleFullscreen() {
    const el = rootEl
    if (!el) return
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen()
      } else {
        await el.requestFullscreen()
      }
    } catch {
      // ignore
    }
  }

  onMount(() => {
    bumpControls()
    if (boot) void load(boot).catch((e) => (lastErr = String(e)))
  })

  onDestroy(() => {
    void destroyInternal()
  })
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div
  class="master-root"
  bind:this={rootEl}
  on:mousemove={bumpControls}
  on:click={bumpControls}
  on:focusin={bumpControls}
>
  <div class="master-stage" bind:this={stageEl}></div>

  <div class="master-controls" data-visible={showControls}>
    <div class="master-top">
      <div class="master-title">{boot?.title || '播放器'}</div>
      <div class="master-sub">{boot ? `${boot.site} / ${boot.room_id} / ${boot.variant_label}` : ''}</div>
    </div>

    <div class="master-bottom">
      <button class="btn" on:click|stopPropagation={togglePlay}>{playing ? '暂停' : '播放'}</button>
      <button class="btn" on:click|stopPropagation={toggleMute}>{muted ? '取消静音' : '静音'}</button>
      <input
        class="vol"
        type="range"
        min="0"
        max="100"
        step="1"
        value={volume}
        on:input|stopPropagation={onVolumeInput}
      />
      <button class="btn" on:click|stopPropagation={toggleFullscreen}>全屏</button>
    </div>
  </div>

  {#if lastErr}
    <div class="master-error">{lastErr}</div>
  {/if}
</div>

<style>
  .master-root {
    position: relative;
    width: 100%;
    height: 100%;
    background: #000;
    overflow: hidden;
  }

  .master-stage {
    position: absolute;
    inset: 0;
  }

  .master-controls {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    pointer-events: none;
    opacity: 0;
    transition: opacity 140ms ease-out;
  }

  .master-controls[data-visible='true'] {
    opacity: 1;
  }

  .master-top {
    pointer-events: none;
    padding: 10px 12px;
    background: linear-gradient(to bottom, rgba(0, 0, 0, 0.65), rgba(0, 0, 0, 0));
  }

  .master-title {
    color: #fff;
    font-weight: 700;
    font-size: 14px;
    line-height: 1.2;
  }

  .master-sub {
    margin-top: 2px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
  }

  .master-bottom {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.65), rgba(0, 0, 0, 0));
  }

  .btn {
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    cursor: pointer;
  }

  .btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .vol {
    width: 160px;
  }

  .master-error {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 64px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(255, 0, 0, 0.18);
    border: 1px solid rgba(255, 0, 0, 0.35);
    color: #fff;
    font-size: 12px;
    white-space: pre-wrap;
  }
</style>
