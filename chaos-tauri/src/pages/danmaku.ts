import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { el, type Cleanup } from '../shared/dom'
import { getOverlayMode } from '../shared/prefs'
import type { DanmakuUiMessage } from '../shared/types'
import { createDanmakuListStore } from '../danmaku/store'

export async function buildDanmakuPage(root: HTMLElement): Promise<Cleanup | undefined> {
  root.appendChild(el('h2', { className: 'heading', text: '弹幕' }))
  root.appendChild(
    el('div', {
      className: 'text-secondary',
      text: '输入直播间 URL（B 站/斗鱼/虎牙）后点击“解析/连接”，页面下方会以 Chat 方式实时输出弹幕。'
    })
  )

  let connected = false

  const inputGroup = el('div', { className: 'stack gap-6' })
  inputGroup.appendChild(el('div', { className: 'field-label', text: '直播间地址' }))

  const row1 = el('div', { className: 'row gap-12 wrap align-center' })
  const input = el('input', { className: 'input' }) as HTMLInputElement
  input.placeholder =
    '例如：https://live.bilibili.com/1 / https://www.douyu.com/xxx / https://www.huya.com/xxx'
  input.style.minWidth = '520px'

  const connectBtn = el('button', { className: 'button primary', text: '解析/连接' }) as HTMLButtonElement
  connectBtn.type = 'button'
  connectBtn.style.width = '120px'

  const disconnectBtn = el('button', { className: 'button secondary', text: '断开' }) as HTMLButtonElement
  disconnectBtn.type = 'button'
  disconnectBtn.style.width = '92px'

  row1.appendChild(input)
  row1.appendChild(connectBtn)
  row1.appendChild(disconnectBtn)
  inputGroup.appendChild(row1)

  const row2 = el('div', { className: 'row gap-12 wrap align-center' })
  const openChatBtn = el('button', { className: 'button secondary', text: 'Chat 窗口' }) as HTMLButtonElement
  openChatBtn.type = 'button'
  const openOverlayBtn = el('button', {
    className: 'button secondary',
    text: 'Overlay 窗口'
  }) as HTMLButtonElement
  openOverlayBtn.type = 'button'
  row2.appendChild(openChatBtn)
  row2.appendChild(openOverlayBtn)
  inputGroup.appendChild(row2)

  root.appendChild(inputGroup)

  root.appendChild(el('div', { className: 'divider' }))

  const connStatus = el('div', { className: 'text-secondary' })
  const perfStatus = el('div', { className: 'text-muted' })
  root.appendChild(connStatus)
  root.appendChild(perfStatus)

  const panel = el('div', { className: 'panel dm-panel' })
  const empty = el('div', { className: 'empty' })
  empty.textContent = '请先输入直播间地址并点击“解析/连接”。'
  const scroll = el('div', { className: 'dm-scroll' })
  const list = el('div', { className: 'dm-list' })
  scroll.appendChild(list)
  panel.appendChild(empty)
  panel.appendChild(scroll)
  root.appendChild(panel)

  function applyConnectedUi() {
    connectBtn.disabled = connected
    disconnectBtn.disabled = !connected
    openChatBtn.disabled = !connected
    openOverlayBtn.disabled = !connected
    connectBtn.textContent = connected ? '已连接' : '解析/连接'
  }
  applyConnectedUi()

  const store = createDanmakuListStore({
    scrollEl: scroll,
    listEl: list,
    statusEl: perfStatus,
    maxItems: 400,
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

  async function doConnect() {
    perfStatus.textContent = ''
    connStatus.textContent = ''
    try {
      await invoke('danmaku_connect', { input: input.value })
      connected = true
      empty.textContent = '已连接：等待弹幕...'
      applyConnectedUi()
    } catch (e) {
      connected = false
      applyConnectedUi()
      connStatus.textContent = `连接失败：${String(e)}`
    }
  }

  async function doDisconnect() {
    perfStatus.textContent = ''
    try {
      await invoke('danmaku_disconnect')
    } catch (e) {
      connStatus.textContent = `断开失败：${String(e)}`
    } finally {
      connected = false
      applyConnectedUi()
      store.clear()
      empty.textContent = '请先输入直播间地址并点击“解析/连接”。'
      empty.style.display = ''
    }
  }

  connectBtn.onclick = () => void doConnect()
  disconnectBtn.onclick = () => void doDisconnect()
  input.onkeydown = (ev) => {
    if (ev.key === 'Enter') void doConnect()
  }

  openChatBtn.onclick = () => void invoke('open_chat_window')
  openOverlayBtn.onclick = () => {
    const opaque = getOverlayMode() === 'opaque'
    return void invoke('open_overlay_window', { opaque })
  }

  const unStatus = await listen<string>('danmaku_status', (e) => {
    connStatus.textContent = e.payload
    const s = (e.payload || '').toString()
    if (s.includes('已连接')) {
      connected = true
      empty.textContent = list.childElementCount > 0 ? '' : '已连接：等待弹幕...'
      applyConnectedUi()
    }
    if (s.includes('已断开')) {
      connected = false
      applyConnectedUi()
    }
  })

  const unMsg = await listen<DanmakuUiMessage>('danmaku_msg', (e) => {
    store.enqueue(e.payload)
  })

  return () => {
    unStatus()
    unMsg()
    store.dispose()
  }
}

