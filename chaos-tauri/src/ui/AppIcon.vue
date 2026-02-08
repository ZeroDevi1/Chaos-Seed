<script setup lang="ts">
import { computed } from 'vue'

import { ICONS, type AppIconName } from './icons'

const props = withDefaults(
  defineProps<{
    name: AppIconName
    size?: number
    title?: string
  }>(),
  {
    size: 20,
    title: ''
  }
)

const svg = computed(() => ICONS[props.name])
</script>

<template>
  <span
    class="app-icon"
    :style="{ width: `${props.size}px`, height: `${props.size}px` }"
    :title="props.title || undefined"
    aria-hidden="true"
    v-html="svg"
  />
</template>

<style scoped>
.app-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: inherit;
}

.app-icon :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.app-icon :deep(path) {
  fill: currentColor;
}
</style>

