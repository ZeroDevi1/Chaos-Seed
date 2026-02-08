<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'

import type { AppInfo } from '@/shared/types'

const version = ref<string>('加载中...')
const homepage = ref<string>('')
const homepageErr = ref<string>('')

onMounted(async () => {
  try {
    const info = await invoke<AppInfo>('get_app_info')
    version.value = `v${info.version}`
    homepage.value = info.homepage
  } catch (e) {
    homepageErr.value = String(e)
    version.value = '获取失败'
  }
})

async function openHomepage() {
  if (!homepage.value) return
  try {
    await invoke('open_url', { url: homepage.value })
  } catch {
    // ignore
  }
}
</script>

<template>
  <div class="page page-narrow">
    <h2 class="heading">关于</h2>

    <fluent-card class="app-card">
      <div class="card-pad stack gap-12">
        <div class="text-secondary">版本：{{ version }}</div>

        <div class="row gap-8 wrap align-center">
          <div class="text-secondary">项目地址：</div>
          <fluent-button v-if="homepage" appearance="stealth" @click="openHomepage">
            {{ homepage }}
          </fluent-button>
          <div v-else class="text-muted">（获取失败：{{ homepageErr }}）</div>
        </div>
      </div>
    </fluent-card>
  </div>
</template>
