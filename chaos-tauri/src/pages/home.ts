import { el, type Cleanup } from '../shared/dom'

export async function buildHomePage(
  root: HTMLElement,
  opts: { goSubtitle: () => void }
): Promise<Cleanup | undefined> {
  root.appendChild(el('h1', { className: 'title', text: 'Chaos Seed' }))
  root.appendChild(
    el('div', {
      className: 'text-secondary',
      text: '在迅雷字幕接口中搜索并下载字幕（Rust + Tauri）。'
    })
  )
  root.appendChild(
    el('div', {
      className: 'text-muted',
      text: '推荐流程：输入关键词 -> 搜索 -> 点击单条下载 -> 选择目录。'
    })
  )

  const btn = el('button', { className: 'button primary', text: '前往字幕下载' }) as HTMLButtonElement
  btn.type = 'button'
  btn.onclick = () => opts.goSubtitle()
  root.appendChild(btn)

  return undefined
}

