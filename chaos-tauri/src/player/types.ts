import type { StreamVariant } from '@/shared/livestreamTypes'

export type PlayerBootRequest = {
  site: string
  room_id: string
  title: string
  cover?: string | null
  variant_id: string
  variant_label: string
  url: string
  backup_urls: string[]
  referer?: string | null
  user_agent?: string | null
  variants?: StreamVariant[] | null
}

export type PlayerEngineKind = 'native' | 'hls' | 'avplayer'

export type PlayerSource = {
  url: string
  backup_urls: string[]
  isLive: boolean
  kind: PlayerEngineKind
  referer?: string | null
  user_agent?: string | null
}

export type PlayerEngine = {
  kind: PlayerEngineKind
  init: (container: HTMLElement) => Promise<void>
  load: (source: PlayerSource) => Promise<void>
  play: () => Promise<void>
  pause: () => Promise<void> | void
  setMuted: (muted: boolean) => void
  setVolume: (volume01: number) => void
  destroy: () => Promise<void> | void
}
