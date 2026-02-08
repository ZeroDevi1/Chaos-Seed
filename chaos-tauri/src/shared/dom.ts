export type Cleanup = () => void

export function clear(el: HTMLElement) {
  while (el.firstChild) el.removeChild(el.firstChild)
}

export function mount(node: HTMLElement) {
  const root = document.getElementById('app')
  if (!root) throw new Error('#app not found')
  root.innerHTML = ''
  root.appendChild(node)
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  opts: {
    className?: string
    text?: string
    attrs?: Record<string, string>
  } = {}
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag)
  if (opts.className) node.className = opts.className
  if (opts.text !== undefined) node.textContent = opts.text
  if (opts.attrs) {
    for (const [k, v] of Object.entries(opts.attrs)) node.setAttribute(k, v)
  }
  return node
}

export function divider(): HTMLElement {
  return el('div', { className: 'divider' })
}

