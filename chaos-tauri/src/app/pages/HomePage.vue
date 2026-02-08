<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()
</script>

<template>
  <div class="page page-narrow">
    <h1 class="title">Chaos Seed</h1>

    <div class="text-secondary">在迅雷字幕接口中搜索并下载字幕（Rust + Tauri）。</div>
    <div class="text-muted">推荐流程：输入关键词 -> 搜索 -> 点击单条下载 -> 选择目录。</div>

    <fluent-card class="app-card">
      <div class="card-pad">
        <fluent-button appearance="accent" @click="router.push('/subtitle')">前往字幕下载</fluent-button>
      </div>
    </fluent-card>
  </div>
</template>
