import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { el, mount } from '../shared/dom'
import type { DanmakuUiMessage } from '../shared/types'
import { initTheme } from '../ui/theme'
import { createDanmakuListStore } from '../danmaku/store'

export async function buildChatWindow(): Promise<void> {
  initTheme()

  const root = el('div', { className: 'window-root' })
  root.appendChild(el('h2', { className: 'heading', text: '弹幕 - Chat' }))

  const connStatus = el('div', { className: 'text-secondary' })
  const perfStatus = el('div', { className: 'text-muted' })
  root.appendChild(connStatus)
  root.appendChild(perfStatus)

  const panel = el('div', { className: 'panel dm-panel' })
  const empty = el('div', { className: 'empty', text: '等待弹幕...' })
  const scroll = el('div', { className: 'dm-scroll' })
  const list = el('div', { className: 'dm-list' })
  scroll.appendChild(list)
  panel.appendChild(empty)
  panel.appendChild(scroll)
  root.appendChild(panel)

  mount(root)

  const store = createDanmakuListStore({
    scrollEl: scroll,
    listEl: list,
    statusEl: perfStatus,
    maxItems: 300,
    onOpenUrl: async (url: string) => {
      try {
        await invoke('open_url', { url })
      } catch {
        // ignore
      }
    },
    afterFlush: (count) => {
      empty.style.display = count > 0 ? 'none' : ''
    }
  })

  const unStatus = await listen<string>('danmaku_status', (e) => {
    connStatus.textContent = e.payload
  })
  const unMsg = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
    store.enqueue(e.payload)
  })

  window.addEventListener('beforeunload', () => {
    unStatus()
    unMsg()
    store.dispose()
  })
}

