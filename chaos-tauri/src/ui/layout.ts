import { el } from '../shared/dom'
import { getSidebarCollapsed, setSidebarCollapsed } from '../shared/prefs'
import { ThemeController } from './theme'

export type NavIndex = 0 | 1 | 2 | 3 | 4 | 5

type NavItem = {
  index: NavIndex
  icon: string
  text: string
}

const NAV: NavItem[] = [
  { index: 0, icon: '⌂', text: '首页' },
  { index: 1, icon: '⬇', text: '字幕下载' },
  { index: 2, icon: '▶', text: '直播源' },
  { index: 3, icon: '≋', text: '弹幕' },
  { index: 4, icon: '⚙', text: '设置' },
  { index: 5, icon: 'ⓘ', text: '关于' }
]

export type Layout = {
  root: HTMLElement
  content: HTMLElement
  getPageIndex(): NavIndex
  setPageIndex(i: NavIndex): void
  onPageChange(cb: (i: NavIndex) => void): () => void
}

export function createLayout(theme: ThemeController): Layout {
  let pageIndex: NavIndex = 0
  const listeners = new Set<(i: NavIndex) => void>()

  const root = el('div', { className: 'app-root' })

  const sidebar = el('div', { className: 'sidebar' })
  const mainCol = el('div', { className: 'main-col' })
  const topbar = el('div', { className: 'topbar' })
  const content = el('div', { className: 'content' })

  root.appendChild(sidebar)
  root.appendChild(mainCol)
  mainCol.appendChild(topbar)
  mainCol.appendChild(content)

  // Sidebar header (collapse)
  const header = el('div', { className: 'sidebar-header' })
  const headerBtn = el('button', { className: 'sidebar-header-btn' }) as HTMLButtonElement
  headerBtn.type = 'button'
  const headerIcon = el('span', { className: 'sidebar-icon', text: '≡' })
  const headerText = el('span', { className: 'sidebar-title', text: 'Chaos Seed' })
  headerBtn.appendChild(headerIcon)
  headerBtn.appendChild(headerText)
  header.appendChild(headerBtn)
  sidebar.appendChild(header)

  const nav = el('div', { className: 'sidebar-nav' })
  sidebar.appendChild(nav)

  const sep = el('div', { className: 'sidebar-sep' })
  const navBottom = el('div', { className: 'sidebar-nav' })
  sidebar.appendChild(el('div', { className: 'sidebar-spacer' }))
  sidebar.appendChild(sep)
  sidebar.appendChild(navBottom)

  const itemEls = new Map<NavIndex, HTMLElement>()

  function mkItem(item: NavItem): HTMLElement {
    const wrap = el('div', { className: 'sidebar-item' })
    const indicator = el('div', { className: 'sidebar-indicator' })
    const icon = el('span', { className: 'sidebar-icon', text: item.icon })
    const text = el('span', { className: 'sidebar-text', text: item.text })
    wrap.appendChild(indicator)
    wrap.appendChild(icon)
    wrap.appendChild(text)
    wrap.onclick = () => setPageIndex(item.index)
    itemEls.set(item.index, wrap)
    return wrap
  }

  // Top items
  for (const item of NAV.filter((x) => x.index <= 3)) nav.appendChild(mkItem(item))
  // Bottom items
  for (const item of NAV.filter((x) => x.index >= 4)) navBottom.appendChild(mkItem(item))

  function applySelected() {
    for (const [idx, node] of itemEls) {
      node.classList.toggle('selected', idx === pageIndex)
    }
  }

  // Collapsed state
  let collapsed = getSidebarCollapsed()
  function applyCollapsed() {
    sidebar.classList.toggle('collapsed', collapsed)
  }
  applyCollapsed()
  headerBtn.onclick = () => {
    collapsed = !collapsed
    setSidebarCollapsed(collapsed)
    applyCollapsed()
  }

  // Topbar: theme select
  const topbarRight = el('div', { className: 'topbar-right' })
  const themeSel = el('select', { className: 'select' }) as HTMLSelectElement
  themeSel.appendChild(new Option('跟随系统', 'system'))
  themeSel.appendChild(new Option('浅色主题', 'light'))
  themeSel.appendChild(new Option('深色主题', 'dark'))
  themeSel.value = theme.getMode()
  themeSel.onchange = () => theme.setMode(themeSel.value as any)
  theme.onChange((m) => {
    themeSel.value = m
  })
  topbarRight.appendChild(themeSel)
  topbar.appendChild(el('div', { className: 'topbar-spacer' }))
  topbar.appendChild(topbarRight)

  function setPageIndex(i: NavIndex) {
    if (i === pageIndex) return
    pageIndex = i
    applySelected()
    for (const cb of listeners) cb(i)
  }

  applySelected()

  return {
    root,
    content,
    getPageIndex: () => pageIndex,
    setPageIndex,
    onPageChange: (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    }
  }
}

