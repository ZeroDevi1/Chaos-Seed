<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'

  import type { AppInfo } from '@/shared/types'

  let version = '加载中...'
  let homepage = ''
  let homepageErr = ''

  onMount(() => {
    let disposed = false
    void (async () => {
      try {
        const info = await invoke<AppInfo>('get_app_info')
        if (disposed) return
        version = `v${info.version}`
        homepage = info.homepage
      } catch (e) {
        if (disposed) return
        homepageErr = String(e)
        version = '获取失败'
      }
    })()
    return () => {
      disposed = true
    }
  })

  async function openHomepage() {
    if (!homepage) return
    try {
      await invoke('open_url', { url: homepage })
    } catch {
      // ignore
    }
  }
</script>

<div class="page page-narrow">
  <h2 class="heading">关于</h2>

  <fluent-card class="app-card">
    <div class="card-pad stack gap-12">
      <div class="text-secondary">版本：{version}</div>

      <div class="row gap-8 wrap align-center">
        <div class="text-secondary">项目地址：</div>
        {#if homepage}
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <fluent-button appearance="stealth" on:click={openHomepage}>
            {homepage}
          </fluent-button>
        {:else}
          <div class="text-muted">（获取失败：{homepageErr}）</div>
        {/if}
      </div>
    </div>
  </fluent-card>
</div>
