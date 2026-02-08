import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

import HomePage from './pages/HomePage.vue'
import SubtitleDownloadPage from './pages/SubtitleDownloadPage.vue'
import LiveSourcePage from './pages/LiveSourcePage.vue'
import DanmakuPage from './pages/DanmakuPage.vue'
import SettingsPage from './pages/SettingsPage.vue'
import AboutPage from './pages/AboutPage.vue'

const routes: RouteRecordRaw[] = [
  { path: '/', component: HomePage, meta: { keepAlive: true } },
  { path: '/subtitle', component: SubtitleDownloadPage, meta: { keepAlive: true } },
  { path: '/live-source', component: LiveSourcePage, meta: { keepAlive: true } },
  // Danmaku page manages event listeners; don't keep it alive to avoid background subscriptions.
  { path: '/danmaku', component: DanmakuPage },
  { path: '/settings', component: SettingsPage, meta: { keepAlive: true } },
  { path: '/about', component: AboutPage, meta: { keepAlive: true } }
]

export function createAppRouter() {
  return createRouter({
    history: createWebHashHistory(),
    routes
  })
}
