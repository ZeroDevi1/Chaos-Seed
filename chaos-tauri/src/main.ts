import './style.css'

import './ui/fluent'

import { createApp } from 'vue'
import { createPinia } from 'pinia'

import MainApp from './app/MainApp.vue'
import ChatApp from './app/ChatApp.vue'
import OverlayApp from './app/OverlayApp.vue'
import { createAppRouter } from './app/router'
import { installDisableZoom } from './ui/disableZoom'
import { applyEarlyTheme } from './ui/earlyTheme'

function viewFromQuery(): 'main' | 'chat' | 'overlay' {
  const params = new URLSearchParams(window.location.search)
  const view = (params.get('view') || '').toLowerCase()
  if (view === 'chat') return 'chat'
  if (view === 'overlay') return 'overlay'
  return 'main'
}

function mountError(e: unknown) {
  const root = document.getElementById('app')
  if (!root) return
  const pre = document.createElement('pre')
  pre.className = 'page'
  pre.textContent = String(e)
  root.innerHTML = ''
  root.appendChild(pre)
}

async function main() {
  applyEarlyTheme()

  // Reduce "WebView app" vibes: disable browser zoom shortcuts in the embedded webview.
  installDisableZoom()

  const view = viewFromQuery()
  const pinia = createPinia()

  if (view === 'chat') {
    createApp(ChatApp).use(pinia).mount('#app')
    return
  }

  if (view === 'overlay') {
    createApp(OverlayApp).use(pinia).mount('#app')
    return
  }

  const router = createAppRouter()
  createApp(MainApp).use(pinia).use(router).mount('#app')
}

main().catch((e) => {
  mountError(e)
})
