<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import type { ThunderSubtitleItem } from '@/shared/types'

const query = ref('')
const minScore = ref<number | null>(null)
const lang = ref('')
const limit = ref<number>(20)

const busy = ref(false)
const status = ref<string>('')
const items = ref<ThunderSubtitleItem[]>([])

function normalizeLimit(v: number | null): number {
  const n = Number.isFinite(v ?? NaN) ? (v as number) : 20
  return Math.max(1, Math.min(200, Math.floor(n)))
}

function readStringValue(ev: Event): string {
  return (ev.target as unknown as { value: string }).value ?? ''
}

function readOptNumberValue(ev: Event): number | null {
  const raw = (ev.target as unknown as { value: string | number | null | undefined }).value
  const s = String(raw ?? '').trim()
  if (!s) return null
  const n = Number(s)
  return Number.isFinite(n) ? n : null
}

function onQueryInput(ev: Event) {
  query.value = readStringValue(ev)
}

function onLangInput(ev: Event) {
  lang.value = readStringValue(ev)
}

function onMinScoreInput(ev: Event) {
  minScore.value = readOptNumberValue(ev)
}

function onLimitInput(ev: Event) {
  const n = readOptNumberValue(ev)
  limit.value = normalizeLimit(n)
}

function onQueryKeyDown(ev: KeyboardEvent) {
  if (ev.key === 'Enter') void doSearch()
}

const hasResults = computed(() => items.value.length > 0)

async function doSearch() {
  const q = query.value.trim()
  if (!q) {
    status.value = '请输入关键词。'
    items.value = []
    return
  }

  busy.value = true
  status.value = '正在搜索...'
  try {
    const out = await invoke<ThunderSubtitleItem[]>('subtitle_search', {
      query: q,
      min_score: minScore.value,
      lang: lang.value.trim() ? lang.value.trim() : null,
      limit: normalizeLimit(limit.value)
    })
    items.value = out
    status.value = `搜索完成：${out.length} 条结果`
  } catch (e) {
    items.value = []
    status.value = `搜索失败：${String(e)}`
  } finally {
    busy.value = false
  }
}

async function downloadItem(it: ThunderSubtitleItem) {
  const picked = await open({ directory: true, multiple: false })
  if (!picked || Array.isArray(picked)) return
  status.value = `下载中 -> ${picked} ...`
  try {
    const out = await invoke<string>('subtitle_download', {
      item: it,
      out_dir: picked,
      overwrite: false
    })
    status.value = `完成：${out}`
  } catch (e) {
    status.value = `下载失败：${String(e)}`
  }
}
</script>

<template>
  <div class="page page-wide">
    <h2 class="heading">字幕下载</h2>
    <div class="text-secondary">
      使用说明：输入关键词（回车或点击搜索）-> 列表展示 -> 点击某条“下载” -> 选择保存目录 -> 开始下载。
    </div>

    <fluent-card class="app-card">
      <div class="card-pad stack gap-12">
        <div class="stack gap-6">
          <div class="field-label">关键词</div>
          <div class="row gap-12 wrap align-center">
            <fluent-text-field
              class="input field-query"
              placeholder="例如：泽塔奥特曼 / 电影名 / 剧名（回车搜索）"
              :value="query"
              :disabled="busy"
              @input="onQueryInput"
              @keydown="onQueryKeyDown"
            />
            <fluent-button class="w-120" appearance="accent" :disabled="busy" @click="doSearch">
              {{ busy ? '处理中...' : '搜索' }}
            </fluent-button>
          </div>
        </div>

        <div class="row gap-12 wrap align-center">
          <div class="stack gap-6">
            <div class="field-label">最低分数(min_score，可空)</div>
            <fluent-number-field
              class="input w-180"
              placeholder="例如：50"
              :value="minScore === null ? '' : String(minScore)"
              :disabled="busy"
              min="0"
              @input="onMinScoreInput"
            />
          </div>

          <div class="stack gap-6">
            <div class="field-label">语言(lang，可空)</div>
            <fluent-text-field
              class="input w-160"
              placeholder="例如：zh / en"
              :value="lang"
              :disabled="busy"
              @input="onLangInput"
            />
          </div>

          <div class="stack gap-6">
            <div class="field-label">数量(limit)</div>
            <fluent-number-field
              class="input w-120"
              placeholder="默认 20"
              :value="String(limit)"
              :disabled="busy"
              min="1"
              max="200"
              @input="onLimitInput"
            />
          </div>
        </div>

        <div class="text-muted">
          提示：搜索后每条结果右侧都有“下载”按钮；点击后会弹出目录选择（每次下载都需要选择目录）。
        </div>
      </div>
    </fluent-card>

    <div class="text-secondary">{{ status }}</div>

    <div class="panel results-panel">
      <div v-if="!hasResults" class="empty">
        {{ busy ? '正在搜索...' : '输入关键词后点击“搜索”。' }}
      </div>

      <div v-else class="results-scroll">
        <div class="result-head">
          <div class="col-score">分数</div>
          <div class="col-name">名称</div>
          <div class="col-ext">格式</div>
          <div class="col-lang">语言</div>
          <div class="col-act" />
        </div>

        <div class="results-list">
          <div v-for="(it, idx) in items" :key="idx" class="result-row">
            <div class="col-score">{{ it.score.toFixed(2) }}</div>
            <div class="col-name" :title="it.name">{{ it.name }}</div>
            <div class="col-ext">{{ it.ext?.trim() ? it.ext : 'srt' }}</div>
            <div class="col-lang" :title="(it.languages || []).filter(Boolean).join(',')">
              {{ (it.languages || []).filter(Boolean).join(',') }}
            </div>
            <div class="col-act">
              <fluent-button appearance="outline" class="w-92" :disabled="busy" @click="downloadItem(it)">
                下载
              </fluent-button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
