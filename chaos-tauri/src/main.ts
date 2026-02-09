import './style.css'

import './ui/fluent'

import { invoke } from '@tauri-apps/api/core'
import { mount, unmount } from 'svelte'

import MainApp from './app/MainApp.svelte'
import ChatApp from './app/ChatApp.svelte'
import OverlayApp from './app/OverlayApp.svelte'
import { resolveView } from './shared/bootView'
import { prefs, resolvedTheme } from './stores/prefs'
import type { BackdropMode } from './shared/prefs'
import { installDisableZoom } from './ui/disableZoom'
import { applyEarlyTheme } from './ui/earlyTheme'
import { applyFluentTokens } from './ui/fluent'

function mountError(e: unknown) {
  const root = document.getElementById('app')
  if (!root) return
  const pre = document.createElement('pre')
  pre.className = 'page'
  pre.textContent = e instanceof Error ? (e.stack || e.message || String(e)) : String(e)
  root.innerHTML = ''
  root.appendChild(pre)
}

function applyBackdrop(mode: BackdropMode, view: 'main' | 'chat' | 'overlay') {
  if (view === 'overlay') {
    document.documentElement.dataset.backdrop = 'none'
    return
  }
  document.documentElement.dataset.backdrop = mode
  // Best-effort: apply OS-side window effects on Windows. Non-Windows just no-ops.
  void invoke('set_backdrop', { mode }).catch(() => {})
}

async function main() {
  applyEarlyTheme()

  // Reduce "WebView app" vibes: disable browser zoom shortcuts in the embedded webview.
  installDisableZoom()

  let label: string | undefined
  try {
    const api = await import('@tauri-apps/api/webviewWindow')
    label = api.getCurrentWebviewWindow().label
  } catch {
    label = undefined
  }
  const view = resolveView({ boot: window.__CHAOS_SEED_BOOT, search: window.location.search, label })

  const stopSystemSync = prefs.startSystemThemeSync()
  const stopCrossSync = prefs.startCrossWindowSync()

  const unTheme = resolvedTheme.subscribe((theme) => {
    document.documentElement.dataset.theme = theme
    applyFluentTokens(theme)
  })

  let prevBackdrop: BackdropMode | null = null
  const unBackdrop = prefs.subscribe((s) => {
    if (view === 'overlay') return
    if (prevBackdrop === s.backdropMode) return
    prevBackdrop = s.backdropMode
    applyBackdrop(s.backdropMode, view)
  })
  // Ensure overlay never tries to use backdrop CSS.
  if (view === 'overlay') applyBackdrop('none', view)

  const root = document.getElementById('app')
  if (!root) throw new Error('#app not found')

  let app: unknown = null
  const cleanup = () => {
    stopSystemSync()
    stopCrossSync()
    unTheme()
    unBackdrop()
    try {
      if (app) unmount(app)
    } catch {
      // ignore
    }
  }
  window.addEventListener('beforeunload', cleanup, { capture: true, once: true })

  if (view === 'chat') {
    app = mount(ChatApp, { target: root })
    return
  }

  if (view === 'overlay') {
    app = mount(OverlayApp, { target: root })
    return
  }

  app = mount(MainApp, { target: root })
}

main().catch((e) => {
  mountError(e)
})
