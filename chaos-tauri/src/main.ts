import './style.css'

import './ui/fluent'

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { mount, unmount } from 'svelte'

import MainApp from './app/MainApp.svelte'
import ChatApp from './app/ChatApp.svelte'
import OverlayApp from './app/OverlayApp.svelte'
import PlayerApp from './app/PlayerApp.svelte'
import LyricsChatApp from './app/LyricsChatApp.svelte'
import LyricsOverlayApp from './app/LyricsOverlayApp.svelte'
import LyricsDockApp from './app/LyricsDockApp.svelte'
import LyricsFloatApp from './app/LyricsFloatApp.svelte'
import { resolveView } from './shared/bootView'
import { windowPresence } from './stores/windowPresence'
import { prefs, resolvedTheme } from './stores/prefs'
import type { BackdropMode } from './shared/prefs'
import type { BootView } from './shared/bootView'
import { installDisableZoom } from './ui/disableZoom'
import { applyEarlyTheme } from './ui/earlyTheme'
import { applyFluentTokens, initSystemAccent } from './ui/fluent'

function mountError(e: unknown) {
  const root = document.getElementById('app')
  if (!root) return
  const pre = document.createElement('pre')
  pre.className = 'page'
  pre.textContent = e instanceof Error ? (e.stack || e.message || String(e)) : String(e)
  root.innerHTML = ''
  root.appendChild(pre)
}

function applyBackdrop(mode: BackdropMode, view: BootView) {
  if (view === 'overlay' || view === 'player' || view === 'lyrics_overlay' || view === 'lyrics_dock' || view === 'lyrics_float') {
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

  // Best-effort: sync Fluent accent tokens to the Windows system accent color.
  // If this fails (non-Windows/dev mode), the app keeps the fallback accent.
  await initSystemAccent()
  applyFluentTokens(document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light')

  let unWindowState: (() => void) | undefined
  void (async () => {
    try {
      const un = await listen<{ label: string; open: boolean }>('chaos_window_state', (e) => {
        windowPresence.setOpen(e.payload.label, e.payload.open)
      })
      unWindowState = un
    } catch {
      // ignore
    }
  })()

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
    if (view === 'overlay' || view === 'player' || view === 'lyrics_overlay' || view === 'lyrics_dock' || view === 'lyrics_float') return
    if (prevBackdrop === s.backdropMode) return
    prevBackdrop = s.backdropMode
    applyBackdrop(s.backdropMode, view)
  })
  // Ensure overlay/player never tries to use backdrop CSS.
  if (view === 'overlay' || view === 'player' || view === 'lyrics_overlay' || view === 'lyrics_dock' || view === 'lyrics_float') applyBackdrop('none', view)

  const root = document.getElementById('app')
  if (!root) throw new Error('#app not found')

  let app: unknown = null
  const cleanup = () => {
    stopSystemSync()
    stopCrossSync()
    unTheme()
    unBackdrop()
    unWindowState?.()
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

  if (view === 'player') {
    app = mount(PlayerApp, { target: root })
    return
  }

  if (view === 'lyrics_chat') {
    app = mount(LyricsChatApp, { target: root })
    return
  }

  if (view === 'lyrics_overlay') {
    app = mount(LyricsOverlayApp, { target: root })
    return
  }

  if (view === 'lyrics_dock') {
    app = mount(LyricsDockApp, { target: root })
    return
  }

  if (view === 'lyrics_float') {
    app = mount(LyricsFloatApp, { target: root })
    return
  }

  app = mount(MainApp, { target: root })
}

main().catch((e) => {
  mountError(e)
})
