import { el, type Cleanup } from '../shared/dom'
import { getOverlayMode, setOverlayMode, type OverlayMode } from '../shared/prefs'
import type { ThemeController } from '../ui/theme'

export async function buildSettingsPage(
  root: HTMLElement,
  opts: { theme: ThemeController }
): Promise<Cleanup | undefined> {
  root.appendChild(el('h2', { className: 'heading', text: '设置' }))

  root.appendChild(
    el('div', {
      className: 'text-secondary',
      text: '提示：主题/侧边栏折叠状态/Overlay 模式会自动持久化。'
    })
  )

  const card = el('div', { className: 'card stack gap-12' })

  // Theme
  const themeRow = el('div', { className: 'row gap-12 wrap align-center' })
  themeRow.appendChild(el('div', { className: 'field-label', text: '主题' }))
  const themeSel = el('select', { className: 'select' }) as HTMLSelectElement
  themeSel.appendChild(new Option('跟随系统', 'system'))
  themeSel.appendChild(new Option('浅色主题', 'light'))
  themeSel.appendChild(new Option('深色主题', 'dark'))
  themeSel.value = opts.theme.getMode()
  themeSel.onchange = () => opts.theme.setMode(themeSel.value as any)
  const un = opts.theme.onChange((m) => {
    themeSel.value = m
  })
  themeRow.appendChild(themeSel)
  card.appendChild(themeRow)

  // Overlay mode
  const overlayRow = el('div', { className: 'row gap-12 wrap align-center' })
  overlayRow.appendChild(el('div', { className: 'field-label', text: 'Overlay 模式' }))
  const overlaySel = el('select', { className: 'select' }) as HTMLSelectElement
  overlaySel.appendChild(new Option('透明（可能不稳定）', 'transparent'))
  overlaySel.appendChild(new Option('不透明（更稳，推荐）', 'opaque'))
  overlaySel.value = getOverlayMode()
  overlaySel.onchange = () => setOverlayMode(overlaySel.value as OverlayMode)
  overlayRow.appendChild(overlaySel)
  card.appendChild(overlayRow)

  card.appendChild(
    el('div', {
      className: 'text-muted',
      text: '说明：透明 Overlay 在某些机器/驱动下可能更吃性能；如果出现掉帧/卡顿，改为“不透明（更稳）”。'
    })
  )

  root.appendChild(card)

  return () => {
    un()
  }
}
