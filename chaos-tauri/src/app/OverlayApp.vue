<script setup lang="ts">
import { onMounted, onBeforeUnmount, watch } from 'vue'

import OverlayWindow from './windows/OverlayWindow.vue'
import { usePrefsStore } from '@/stores/prefs'
import { applyFluentTokens } from '@/ui/fluent'

const prefs = usePrefsStore()

let stopSystemSync: (() => void) | undefined
let stopCrossSync: (() => void) | undefined
onMounted(() => {
  stopSystemSync = prefs.startSystemThemeSync()
  stopCrossSync = prefs.startCrossWindowSync()
})
onBeforeUnmount(() => {
  stopSystemSync?.()
  stopCrossSync?.()
})

watch(
  () => prefs.resolvedTheme,
  (theme) => {
    document.documentElement.dataset.theme = theme
    applyFluentTokens(theme)
  },
  { immediate: true }
)
</script>

<template>
  <OverlayWindow />
</template>
