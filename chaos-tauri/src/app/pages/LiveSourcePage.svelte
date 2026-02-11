<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

  import { fetchDanmakuImage } from '@/shared/danmakuApi'
  import { decodeManifest, resolveVariant } from '@/shared/livestreamApi'
  import type { LiveManifest, StreamVariant } from '@/shared/livestreamTypes'
  import { mergeVariant } from '@/shared/livestreamUtils'
  import type { PlayerBootRequest } from '@/player/types'
  import { calcHeroFromRect, type WindowRect } from './liveSourceHeroRect'

  let input = ''
  let loading = false
  let resolvingId = ''
  let err = ''

  let manifest: LiveManifest | null = null
  let coverBroken = false
  let coverObjectUrl: string | null = null
  let coverKey = ''

  $: variants = manifest?.variants ?? []
  $: void syncCover(manifest)

  function revokeCoverObjectUrl() {
    if (!coverObjectUrl) return
    try {
      URL.revokeObjectURL(coverObjectUrl)
    } catch {
      // ignore
    }
    coverObjectUrl = null
  }

  onDestroy(() => {
    revokeCoverObjectUrl()
  })

  async function syncCover(m: LiveManifest | null) {
    if (!m) {
      coverKey = ''
      coverBroken = false
      revokeCoverObjectUrl()
      return
    }

    const raw = (m.info?.cover ?? '').toString().trim()
    const nextKey = `${m.site}|${m.room_id}|${raw}`
    if (nextKey === coverKey) return
    coverKey = nextKey
    const myKey = nextKey

    coverBroken = false
    revokeCoverObjectUrl()
    if (!raw) return

    try {
      // Prefer proxy-loading via Rust to avoid hotlink/referrer issues (notably bilibili/hdslb).
      const reply = await fetchDanmakuImage({ url: raw, site: m.site, roomId: m.room_id })
      const mime = (reply.mime || '').toString().trim() || 'image/jpeg'
      const buf = new Uint8Array(reply.bytes)
      const blob = new Blob([buf], { type: mime })
      const objectUrl = URL.createObjectURL(blob)
      if (coverKey !== myKey) {
        try {
          URL.revokeObjectURL(objectUrl)
        } catch {
          // ignore
        }
        return
      }
      coverObjectUrl = objectUrl
    } catch {
      // Fallback: the direct URL may still work.
      coverObjectUrl = null
    }
  }

  function onInput(ev: Event) {
    input = ((ev.target as unknown as { value: string })?.value ?? '').toString()
  }

  async function doDecode() {
    err = ''
    manifest = null
    const raw = (input || '').trim()
    if (!raw) return
    loading = true
    try {
      const m = await decodeManifest(raw)
      manifest = m
    } catch (e) {
      err = `解析失败：${String(e)}`
    } finally {
      loading = false
    }
  }

  async function ensureVariantResolved(variantId: string) {
    if (!manifest) return
    const v = manifest.variants.find((x) => x.id === variantId)
    if (!v) return
    const hasUrl = ((v.url ?? '').toString().trim().length > 0)
    if (hasUrl) return

    resolvingId = variantId
    try {
      const resolved = await resolveVariant(manifest.site, manifest.room_id, v.id)
      manifest = mergeVariant(manifest, resolved)
    } catch (e) {
      err = `解析线路失败：${String(e)}`
    } finally {
      resolvingId = ''
    }
  }

  async function calcFromRect(el: HTMLElement): Promise<WindowRect | null> {
    try {
      const win = getCurrentWebviewWindow()
      const pos = await win.innerPosition()
      const rect = el.getBoundingClientRect()
      let sf = 1
      try {
        sf = await win.scaleFactor()
      } catch {
        sf = window.devicePixelRatio || 1
      }
      return calcHeroFromRect({ x: pos.x, y: pos.y }, rect, sf)
    } catch {
      return null
    }
  }

  async function doPlayVariant(v: StreamVariant, cardEl: HTMLElement) {
    if (!manifest) return
    err = ''

    const hasUrl = ((v.url ?? '').toString().trim().length > 0)
    if (!hasUrl) {
      await ensureVariantResolved(v.id)
    }

    const vv = (manifest.variants ?? []).find((x) => x.id === v.id) ?? v
    const url = (vv.url ?? '').toString().trim()
    if (!url) {
      err = '该线路暂无直连 URL'
      return
    }

    const req: PlayerBootRequest = {
      site: manifest.site,
      room_id: manifest.room_id,
      title: manifest.info?.title || 'Live',
      cover: manifest.info?.cover ?? null,
      variant_id: vv.id,
      variant_label: vv.label,
      url,
      backup_urls: vv.backup_urls ?? [],
      referer: manifest.playback?.referer ?? null,
      user_agent: manifest.playback?.user_agent ?? null,
      variants: manifest.variants
    }
    try {
      const fromRect = await calcFromRect(cardEl)
      await invoke('open_player_window', { req, fromRect })
    } catch (e) {
      err = `打开播放器失败：${String(e)}`
    }
  }

  function formatSite(site: string): string {
    const s = (site || '').toString()
    if (s === 'bili_live') return 'B 站直播'
    if (s === 'douyu') return '斗鱼'
    if (s === 'huya') return '虎牙'
    return s || '-'
  }

  function isLivingText(m: LiveManifest | null): string {
    if (!m) return '-'
    return m.info?.is_living ? '直播中' : '未开播/离线'
  }
</script>

<div class="page page-wide">
  <h2 class="heading">直播源</h2>
  <div class="text-secondary">输入直播间 URL（B 站/斗鱼/虎牙）后点击“解析”，选择线路/清晰度并跳转到新窗口播放。</div>

  <fluent-card class="app-card">
    <div class="card-pad stack gap-12">
      <div class="row gap-12 wrap align-center">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <fluent-text-field
          class="input live-url"
          placeholder=""
          value={input}
          on:input={onInput}
          on:keydown={(e: KeyboardEvent) => e.key === 'Enter' && void doDecode()}
        ></fluent-text-field>
        <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
        <fluent-button appearance="accent" class="w-120" disabled={loading || !input.trim()} on:click={doDecode}>
          {loading ? '解析中...' : '解析'}
        </fluent-button>
      </div>

      {#if manifest}
        <div class="divider"></div>

        <div class="grid2">
          <div class="kv">
            <div class="k text-muted">平台</div>
            <div class="v">{formatSite(manifest.site)}</div>
          </div>
          <div class="kv">
            <div class="k text-muted">房间</div>
            <div class="v">{manifest.room_id}</div>
          </div>
          <div class="kv">
            <div class="k text-muted">标题</div>
            <div class="v">{manifest.info?.title || '-'}</div>
          </div>
          <div class="kv">
            <div class="k text-muted">状态</div>
            <div class="v">{isLivingText(manifest)}</div>
          </div>
        </div>

        <div class="variant-grid" aria-label="清晰度/线路">
          {#each variants as v (v.id)}
            <button
              type="button"
              class="variant-card"
              disabled={loading || resolvingId === v.id}
              on:click={(ev) => void doPlayVariant(v, ev.currentTarget as HTMLElement)}
            >
              <div class="variant-thumb">
                {#if coverObjectUrl}
                  <img class="variant-cover" alt="" src={coverObjectUrl} />
                {:else if manifest.info?.cover && !coverBroken}
                  <img class="variant-cover" alt="" src={manifest.info.cover} on:error={() => (coverBroken = true)} />
                {:else}
                  <div class="variant-cover placeholder"></div>
                {/if}
                <div class="variant-badges">
                  <span class="badge">{v.label}</span>
                  {#if !(v.url ?? '').toString().trim()}
                    <span class="badge warn">需解析</span>
                  {/if}
                  {#if resolvingId === v.id}
                    <span class="badge warn">解析中…</span>
                  {/if}
                </div>
              </div>

              <div class="variant-meta">
                <div class="variant-title">{manifest.info?.title || '-'}</div>
                <div class="variant-sub">
                  <span class="muted">{manifest.info?.name || '主播：-'}</span>
                  <span class="muted">·</span>
                  <span class="muted">{isLivingText(manifest)}</span>
                </div>
              </div>
            </button>
          {/each}
        </div>
      {/if}

      {#if err}
        <div class="text-secondary err">{err}</div>
        {#if err.includes('AmbiguousInput') || err.includes('ambiguous')}
          <div class="text-muted">
            提示：可尝试使用前缀明确平台，例如：<code>douyu:xxx</code> / <code>huya:xxx</code> / <code>bili:1</code>
          </div>
        {/if}
      {/if}
    </div>
  </fluent-card>
</div>

<style>
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 16px;
  }
  .kv .k {
    font-size: 12px;
  }
  .kv .v {
    margin-top: 2px;
  }
  .err {
    white-space: pre-wrap;
  }

  .variant-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 12px;
  }

  .variant-card {
    padding: 0;
    border: 1px solid var(--border-color);
    background: var(--card-bg);
    border-radius: 12px;
    overflow: hidden;
    cursor: pointer;
    text-align: left;
    color: inherit;
    display: flex;
    flex-direction: column;
    min-height: 180px;
  }

  .variant-card:hover {
    background: color-mix(in srgb, var(--card-bg) 92%, var(--hover-bg));
  }

  .variant-card:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .variant-thumb {
    position: relative;
    height: 108px;
    background: rgba(0, 0, 0, 0.06);
  }

  .variant-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .variant-cover.placeholder {
    width: 100%;
    height: 100%;
    background: color-mix(in srgb, var(--text-muted) 10%, transparent);
  }

  .variant-badges {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .badge {
    font-size: 12px;
    padding: 4px 8px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.18);
    backdrop-filter: blur(8px);
  }

  .badge.warn {
    background: rgba(255, 165, 0, 0.35);
  }

  .variant-meta {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .variant-title {
    font-weight: 600;
    line-height: 1.2;
  }

  .variant-sub {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 12px;
  }

  .muted {
    color: var(--text-muted);
  }
</style>
