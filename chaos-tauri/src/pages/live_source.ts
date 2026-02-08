import { el, type Cleanup } from '../shared/dom'

export async function buildLiveSourcePage(root: HTMLElement): Promise<Cleanup | undefined> {
  root.appendChild(el('h2', { className: 'heading', text: '直播源' }))
  root.appendChild(
    el('div', {
      className: 'text-secondary',
      text: '占位：后续会在这里提供直播源管理与解析（与 Slint 版本保持一致）。'
    })
  )
  return undefined
}

