import { invoke } from '@tauri-apps/api/core'

import type { NowPlayingSnapshot } from './types'

export async function nowPlayingSnapshot(input?: {
  includeThumbnail?: boolean | null
  maxThumbnailBytes?: number | null
  maxSessions?: number | null
}): Promise<NowPlayingSnapshot> {
  const payload: Record<string, unknown> = {}
  // Tauri commands expect snake_case parameter names.
  if (input?.includeThumbnail !== undefined) payload.include_thumbnail = input.includeThumbnail
  if (input?.maxThumbnailBytes !== undefined) payload.max_thumbnail_bytes = input.maxThumbnailBytes
  if (input?.maxSessions !== undefined) payload.max_sessions = input.maxSessions
  return invoke<NowPlayingSnapshot>('now_playing_snapshot', payload)
}
