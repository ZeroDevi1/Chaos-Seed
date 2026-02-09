export type LiveInfo = {
  title: string
  name?: string | null
  avatar?: string | null
  cover?: string | null
  is_living: boolean
}

export type PlaybackHints = {
  referer?: string | null
  user_agent?: string | null
}

export type StreamVariant = {
  id: string
  label: string
  quality: number
  rate?: number | null
  url?: string | null
  backup_urls: string[]
}

export type LiveManifest = {
  site: string
  room_id: string
  raw_input: string
  info: LiveInfo
  playback: PlaybackHints
  variants: StreamVariant[]
}

