<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  import { decodeManifest, resolveVariant } from '@/shared/livestreamApi'
  import type { LiveManifest, StreamVariant } from '@/shared/livestreamTypes'
  import { mergeVariant, pickDefaultVariant } from '@/shared/livestreamUtils'
  import type { PlayerBootRequest } from '@/player/types'

  let input = ''
  let loading = false
  let resolving = false
  let err = ''

  let manifest: LiveManifest | null = null
  let selectedVariantId = ''

  $: variants = manifest?.variants ?? []
  $: selectedVariant = variants.find((v) => v.id === selectedVariantId) ?? null
  $: urlText = (() => {
    if (!selectedVariant) return ''
    const primary = (selectedVariant.url ?? '').toString().trim()
    const backups = (selectedVariant.backup_urls ?? []).map((u) => (u || '').toString().trim()).filter(Boolean)
    return [primary, ...backups].filter(Boolean).join('\n')
  })()

  function onInput(ev: Event) {
    input = ((ev.target as unknown as { value: string })?.value ?? '').toString()
  }

  async function doDecode() {
    err = ''
    manifest = null
    selectedVariantId = ''
    const raw = (input || '').trim()
    if (!raw) return
    loading = true
    try {
      const m = await decodeManifest(raw)
      manifest = m
      const d = pickDefaultVariant(m)
      selectedVariantId = d?.id ?? (m.variants[0]?.id ?? '')
      // If the chosen variant doesn't have a URL (e.g. Douyu), resolve it immediately for convenience.
      if (selectedVariantId) {
        await ensureVariantResolved(selectedVariantId)
      }
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

    resolving = true
    try {
      const resolved = await resolveVariant(manifest.site, manifest.room_id, v.id)
      manifest = mergeVariant(manifest, resolved)
    } catch (e) {
      err = `解析线路失败：${String(e)}`
    } finally {
      resolving = false
    }
  }

  function onVariantChange(ev: Event) {
    const v = ((ev.target as unknown as { value: string })?.value ?? '').toString()
    selectedVariantId = v
    void ensureVariantResolved(v)
  }

  async function doPlay() {
    if (!manifest || !selectedVariant) return
    const url = (selectedVariant.url ?? '').toString().trim()
    if (!url) return
    const req: PlayerBootRequest = {
      site: manifest.site,
      room_id: manifest.room_id,
      title: manifest.info?.title || 'Live',
      variant_id: selectedVariant.id,
      variant_label: selectedVariant.label,
      url,
      backup_urls: selectedVariant.backup_urls ?? [],
      referer: manifest.playback?.referer ?? null,
      user_agent: manifest.playback?.user_agent ?? null,
      variants: manifest.variants
    }
    try {
      await invoke('open_player_window', { req })
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
          placeholder="例如：https://live.bilibili.com/1 / https://www.douyu.com/xxx / https://www.huya.com/xxx"
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

        <div class="row gap-12 wrap align-center">
          <div class="text-muted w-120">Quality/Line</div>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <fluent-select class="select" value={selectedVariantId} on:change={onVariantChange}>
            {#each variants as v (v.id)}
              <fluent-option value={v.id}>
                {v.label} {v.quality ? `(${v.quality})` : ''}
              </fluent-option>
            {/each}
          </fluent-select>
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <fluent-button appearance="accent" class="w-120" disabled={!selectedVariant?.url || resolving} on:click={doPlay}>
            {resolving ? '解析线路...' : '播放'}
          </fluent-button>
        </div>

        <div class="stack gap-6">
          <div class="text-muted">直连流 URL（调试用）</div>
          <textarea class="raw-url" readonly value={urlText}></textarea>
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
  .raw-url {
    width: 100%;
    min-height: 120px;
    padding: 10px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-color);
    background: var(--input-bg);
    color: var(--text-primary);
    resize: vertical;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
    font-size: 12px;
    line-height: 1.35;
  }
  .err {
    white-space: pre-wrap;
  }
</style>
