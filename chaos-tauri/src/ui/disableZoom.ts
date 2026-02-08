function isZoomShortcut(ev: KeyboardEvent): boolean {
  if (!ev.ctrlKey && !ev.metaKey) return false

  // Common browser zoom shortcuts:
  // - Ctrl/Cmd + '+', '-', '0'
  // - On US keyboards, '+' is often '=' with Shift.
  const k = ev.key
  return k === '+' || k === '-' || k === '0' || k === '='
}

export function installDisableZoom(): () => void {
  function onKeyDown(ev: KeyboardEvent) {
    if (!isZoomShortcut(ev)) return
    ev.preventDefault()
  }

  // Ctrl+Wheel zoom (Chrome/Edge). Must be passive:false to preventDefault.
  function onWheel(ev: WheelEvent) {
    if (!ev.ctrlKey) return
    ev.preventDefault()
  }

  window.addEventListener('keydown', onKeyDown, true)
  window.addEventListener('wheel', onWheel, { capture: true, passive: false })

  return () => {
    window.removeEventListener('keydown', onKeyDown, true)
    window.removeEventListener('wheel', onWheel, true)
  }
}
