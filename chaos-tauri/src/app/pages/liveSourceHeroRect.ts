export type PhysicalPos = { x: number; y: number }

export type DomRectLike = {
  left: number
  top: number
  width: number
  height: number
}

export type WindowRect = { x: number; y: number; width: number; height: number }

export function calcHeroFromRect(innerPos: PhysicalPos, rect: DomRectLike, scaleFactor: number): WindowRect {
  const scale = Number.isFinite(scaleFactor) && scaleFactor > 0 ? scaleFactor : 1
  const width = Math.max(1, Math.round(rect.width * scale))
  const height = Math.max(1, Math.round(rect.height * scale))
  const x = Math.round(innerPos.x + rect.left * scale)
  const y = Math.round(innerPos.y + rect.top * scale)
  return { x, y, width, height }
}
