import { invoke } from '@tauri-apps/api/core'

import { el, type Cleanup } from '../shared/dom'
import type { AppInfo } from '../shared/types'

export async function buildAboutPage(root: HTMLElement): Promise<Cleanup | undefined> {
  root.appendChild(el('h2', { className: 'heading', text: '关于' }))

  const versionLine = el('div', { className: 'text-secondary', text: '版本：加载中...' })
  root.appendChild(versionLine)

  const row = el('div', { className: 'row gap-8 wrap align-center' })
  row.appendChild(el('div', { className: 'text-secondary', text: '项目地址：' }))
  const link = el('a', { className: 'link', text: '加载中...', attrs: { href: '#' } }) as HTMLAnchorElement
  link.onclick = async (ev) => {
    ev.preventDefault()
    const url = link.dataset.url
    if (!url) return
    try {
      await invoke('open_url', { url })
    } catch {
      // ignore; user can still copy it
    }
  }
  row.appendChild(link)
  root.appendChild(row)

  try {
    const info = await invoke<AppInfo>('get_app_info')
    versionLine.textContent = `版本：v${info.version}`
    link.textContent = info.homepage
    link.dataset.url = info.homepage
  } catch (e) {
    versionLine.textContent = `版本：获取失败（${String(e)}）`
    link.textContent = '（获取失败）'
  }

  return undefined
}

