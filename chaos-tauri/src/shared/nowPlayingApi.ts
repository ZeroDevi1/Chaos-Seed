import { invoke } from '@tauri-apps/api/core'

import type { NowPlayingSnapshot } from './types'

export async function nowPlayingSnapshot(input?: {
  includeThumbnail?: boolean | null
  maxThumbnailBytes?: number | null
  maxSessions?: number | null
}): Promise<NowPlayingSnapshot> {
  const payload: Record<string, unknown> = {}
  if (input?.includeThumbnail !== undefined) payload.includeThumbnail = input.includeThumbnail
  if (input?.maxThumbnailBytes !== undefined) payload.maxThumbnailBytes = input.maxThumbnailBytes
  if (input?.maxSessions !== undefined) payload.maxSessions = input.maxSessions
  return invoke<NowPlayingSnapshot>('now_playing_snapshot', payload)
}
