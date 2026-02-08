export type ThunderSubtitleItem = {
  gcid: string
  cid: string
  url: string
  ext: string
  name: string
  duration: number
  languages: string[]
  source: number
  score: number
  fingerprintf_score: number
  extra_name: string
  mt: number
}

export type AppInfo = {
  version: string
  homepage: string
}

export type DanmakuUiMessage = {
  site: string
  room_id: string
  received_at_ms: number
  user: string
  text: string
  image_url?: string | null
  image_width?: number | null
}

