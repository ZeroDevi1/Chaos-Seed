<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { usePrefsStore } from '@/stores/prefs'
import AppIcon from '@/ui/AppIcon.vue'
import type { AppIconName } from '@/ui/icons'

type NavItem = {
  label: string
  path: string
  icon: AppIconName
}

const TOP_ITEMS: NavItem[] = [
  { label: '首页', path: '/', icon: 'home' },
  { label: '字幕下载', path: '/subtitle', icon: 'subtitle' },
  { label: '直播源', path: '/live-source', icon: 'live' },
  { label: '弹幕', path: '/danmaku', icon: 'danmaku' }
]

const BOTTOM_ITEMS: NavItem[] = [
  { label: '设置', path: '/settings', icon: 'settings' },
  { label: '关于', path: '/about', icon: 'about' }
]

const prefs = usePrefsStore()
const route = useRoute()
const router = useRouter()

const selectedPath = computed(() => route.path)

function anchorId(kind: 'top' | 'bottom', path: string) {
  const safe = (path === '/' ? 'root' : path.slice(1)).replaceAll('/', '_') || 'root'
  return `nav-${kind}-${safe}`
}

function toggleSider() {
  prefs.setSidebarCollapsed(!prefs.sidebarCollapsed)
}

function go(path: string) {
  if (path === route.path) return
  router.push(path)
}
</script>

<template>
  <div class="app-root">
    <aside class="sidebar" :class="{ collapsed: prefs.sidebarCollapsed }">
      <div class="sidebar-header">
        <button
          class="sidebar-header-btn"
          type="button"
          :title="prefs.sidebarCollapsed ? '展开' : '折叠'"
          @click="toggleSider"
        >
          <span class="sidebar-icon" aria-hidden="true">≡</span>
          <span class="sidebar-title">Chaos Seed</span>
        </button>
      </div>

      <div class="sidebar-mid">
        <nav class="sidebar-nav" aria-label="导航">
          <template v-for="it in TOP_ITEMS" :key="it.path">
            <button
              :id="anchorId('top', it.path)"
              class="sidebar-item"
              :class="{ selected: selectedPath === it.path }"
              type="button"
              :title="prefs.sidebarCollapsed ? it.label : undefined"
              :aria-current="selectedPath === it.path ? 'page' : undefined"
              @click="go(it.path)"
            >
              <span class="sidebar-indicator" aria-hidden="true" />
              <span class="sidebar-icon" aria-hidden="true">
                <AppIcon :name="it.icon" />
              </span>
              <span class="sidebar-text">{{ it.label }}</span>
            </button>
          </template>
        </nav>
      </div>

      <div class="sidebar-bottom">
        <div class="sidebar-sep" aria-hidden="true" />

        <nav class="sidebar-nav" aria-label="设置">
          <template v-for="it in BOTTOM_ITEMS" :key="it.path">
            <button
              :id="anchorId('bottom', it.path)"
              class="sidebar-item"
              :class="{ selected: selectedPath === it.path }"
              type="button"
              :title="prefs.sidebarCollapsed ? it.label : undefined"
              :aria-current="selectedPath === it.path ? 'page' : undefined"
              @click="go(it.path)"
            >
              <span class="sidebar-indicator" aria-hidden="true" />
              <span class="sidebar-icon" aria-hidden="true">
                <AppIcon :name="it.icon" />
              </span>
              <span class="sidebar-text">{{ it.label }}</span>
            </button>
          </template>
        </nav>
      </div>
    </aside>

    <main class="main-col">
      <div class="content">
        <router-view v-slot="{ Component, route: r }">
          <keep-alive>
            <component :is="Component" v-if="r.meta.keepAlive" />
          </keep-alive>
          <component :is="Component" v-if="!r.meta.keepAlive" />
        </router-view>
      </div>
    </main>
  </div>
</template>
