import { open } from '@tauri-apps/plugin-dialog'

import { clear, el, type Cleanup } from '../shared/dom'
import { subtitleDownload, subtitleSearch } from '../shared/subtitleApi'
import type { ThunderSubtitleItem } from '../shared/types'

function parseOptNum(s: string): number | null {
  const t = s.trim()
  if (!t) return null
  const n = Number(t)
  return Number.isFinite(n) ? n : null
}

function parseLimit(s: string): number {
  const t = s.trim()
  const n = Number(t || '20')
  if (!Number.isFinite(n)) return 20
  return Math.max(1, Math.min(200, Math.floor(n)))
}

export async function buildSubtitleDownloadPage(root: HTMLElement): Promise<Cleanup | undefined> {
  root.appendChild(el('h2', { className: 'heading', text: '字幕下载' }))
  root.appendChild(
    el('div', {
      className: 'text-secondary',
      text: '使用说明：输入关键词（回车或点击搜索）-> 列表展示 -> 点击某条“下载” -> 选择保存目录 -> 开始下载。'
    })
  )

  const form = el('div', { className: 'stack gap-12' })

  // Query row
  const qGroup = el('div', { className: 'stack gap-6' })
  qGroup.appendChild(el('div', { className: 'field-label', text: '关键词' }))
  const qRow = el('div', { className: 'row gap-12' })
  const q = el('input', { className: 'input' }) as HTMLInputElement
  q.placeholder = '例如：泽塔奥特曼 / 电影名 / 剧名（回车搜索）'
  q.style.minWidth = '420px'
  const searchBtn = el('button', { className: 'button primary', text: '搜索' }) as HTMLButtonElement
  searchBtn.type = 'button'
  searchBtn.style.width = '120px'
  qRow.appendChild(q)
  qRow.appendChild(searchBtn)
  qGroup.appendChild(qRow)
  form.appendChild(qGroup)

  // Filters row
  const filters = el('div', { className: 'row gap-12 wrap' })

  function field(label: string, placeholder: string, widthPx: number): HTMLInputElement {
    const g = el('div', { className: 'stack gap-6' })
    g.appendChild(el('div', { className: 'field-label', text: label }))
    const input = el('input', { className: 'input' }) as HTMLInputElement
    input.placeholder = placeholder
    input.style.width = `${widthPx}px`
    g.appendChild(input)
    filters.appendChild(g)
    return input
  }

  const minScore = field('最低分数(min_score，可空)', '例如：50', 180)
  const lang = field('语言(lang，可空)', '例如：zh / en', 160)
  const limit = field('数量(limit)', '默认 20', 120)

  form.appendChild(filters)

  root.appendChild(form)

  root.appendChild(
    el('div', {
      className: 'text-muted',
      text: '提示：搜索后每条结果右侧都有“下载”按钮；点击后会弹出目录选择（每次下载都需要选择目录）。'
    })
  )

  const status = el('div', { className: 'text-secondary' })
  root.appendChild(status)

  const divider = el('div', { className: 'divider' })
  root.appendChild(divider)

  const panel = el('div', { className: 'panel' })
  const empty = el('div', { className: 'empty', text: '输入关键词后点击“搜索”。' })
  const tableWrap = el('div', { className: 'table-wrap' })
  panel.appendChild(empty)
  panel.appendChild(tableWrap)
  root.appendChild(panel)

  let busy = false
  function setBusy(v: boolean) {
    busy = v
    q.disabled = v
    minScore.disabled = v
    lang.disabled = v
    limit.disabled = v
    searchBtn.disabled = v
    searchBtn.textContent = v ? '处理中...' : '搜索'
  }

  function render(items: ThunderSubtitleItem[]) {
    clear(tableWrap)
    if (items.length === 0) {
      empty.textContent = busy ? '正在搜索...' : '暂无结果。'
      empty.style.display = ''
      return
    }
    empty.style.display = 'none'

    const table = el('table', { className: 'table' })
    const thead = el('thead')
    thead.innerHTML = `<tr><th>分数</th><th>名称</th><th>格式</th><th>语言</th><th></th></tr>`
    table.appendChild(thead)
    const tbody = el('tbody')

    for (const it of items) {
      const tr = el('tr')
      tr.appendChild(el('td', { text: it.score.toFixed(2) }))
      tr.appendChild(el('td', { text: it.name }))
      tr.appendChild(el('td', { text: it.ext?.trim() ? it.ext : 'srt' }))
      tr.appendChild(el('td', { text: (it.languages || []).filter(Boolean).join(',') }))

      const tdBtn = el('td')
      const dl = el('button', { className: 'button secondary', text: '下载' }) as HTMLButtonElement
      dl.type = 'button'
      dl.onclick = async () => {
        const picked = await open({ directory: true, multiple: false })
        if (!picked || Array.isArray(picked)) return
        status.textContent = `下载中 -> ${picked} ...`
        try {
          const out = await subtitleDownload({ item: it, outDir: picked, overwrite: false })
          status.textContent = `完成：${out}`
        } catch (e) {
          status.textContent = `下载失败：${String(e)}`
        }
      }
      tdBtn.appendChild(dl)
      tr.appendChild(tdBtn)
      tbody.appendChild(tr)
    }

    table.appendChild(tbody)
    tableWrap.appendChild(table)
  }

  async function doSearch() {
    const query = q.value.trim()
    if (!query) {
      status.textContent = '请输入关键词。'
      render([])
      return
    }
    setBusy(true)
    status.textContent = '正在搜索...'
    try {
      const items = await subtitleSearch({
        query,
        minScore: parseOptNum(minScore.value),
        lang: lang.value.trim() ? lang.value.trim() : null,
        limit: parseLimit(limit.value)
      })
      status.textContent = `搜索完成：${items.length} 条结果`
      render(items)
    } catch (e) {
      status.textContent = `搜索失败：${String(e)}`
      render([])
    } finally {
      setBusy(false)
    }
  }

  searchBtn.onclick = () => void doSearch()
  q.onkeydown = (ev) => {
    if (ev.key === 'Enter') void doSearch()
  }

  return undefined
}
