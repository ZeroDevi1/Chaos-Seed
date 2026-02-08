<script setup lang="ts">
import { usePrefsStore } from '@/stores/prefs'
import type { OverlayMode, ThemeMode } from '@/shared/prefs'

const prefs = usePrefsStore()

function onThemeChange(ev: Event) {
  const v = (ev.target as unknown as { value: string }).value as ThemeMode
  prefs.setThemeMode(v)
}

function onOverlayChange(ev: Event) {
  const v = (ev.target as unknown as { value: string }).value as OverlayMode
  prefs.setOverlayMode(v)
}
</script>

<template>
  <div class="page page-narrow">
    <h2 class="heading">设置</h2>
    <div class="text-secondary">提示：主题/侧边栏折叠状态/Overlay 模式会自动持久化。</div>

    <fluent-card class="app-card">
      <div class="card-pad settings-grid">
        <div class="settings-row">
          <div class="field-label">主题</div>
          <fluent-select class="select" :value="prefs.themeMode" @change="onThemeChange">
            <fluent-option value="system">跟随系统</fluent-option>
            <fluent-option value="light">浅色主题</fluent-option>
            <fluent-option value="dark">深色主题</fluent-option>
          </fluent-select>
        </div>

        <div class="settings-row">
          <div class="field-label">Overlay 模式</div>
          <fluent-select class="select" :value="prefs.overlayMode" @change="onOverlayChange">
            <fluent-option value="transparent">透明（默认）</fluent-option>
            <fluent-option value="opaque">不透明（更稳）</fluent-option>
          </fluent-select>
        </div>

        <div class="text-muted">
          说明：透明 Overlay 在某些机器/驱动下可能更吃性能；如果出现掉帧/卡顿，改为“不透明（更稳）”。
        </div>
      </div>
    </fluent-card>
  </div>
</template>
