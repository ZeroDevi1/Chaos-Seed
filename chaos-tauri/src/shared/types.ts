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

export type NowPlayingThumbnail = {
  mime: string
  base64: string
}

export type NowPlayingSession = {
  app_id: string
  is_current: boolean
  playback_status: string
  title?: string | null
  artist?: string | null
  album_title?: string | null
  position_ms?: number | null
  duration_ms?: number | null
  thumbnail?: NowPlayingThumbnail | null
  error?: string | null
}

export type NowPlayingSnapshot = {
  supported: boolean
  now_playing?: NowPlayingSession | null
  sessions: NowPlayingSession[]
  picked_app_id?: string | null
  retrieved_at_unix_ms: number
}

export type LyricsSearchResult = {
  service: string
  service_token: string
  title?: string | null
  artist?: string | null
  album?: string | null
  duration_ms?: number | null
  quality: number
  matched: boolean
  has_translation: boolean
  has_inline_timetags: boolean
  lyrics_original: string
  lyrics_translation?: string | null
  debug?: unknown | null
}
