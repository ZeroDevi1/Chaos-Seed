import { invoke } from '@tauri-apps/api/core'

export async function nowPlayingSnapshot(input?: {
  includeThumbnail?: boolean | null
  maxThumbnailBytes?: number | null
  maxSessions?: number | null
}): Promise<string> {
  const payload: Record<string, unknown> = {}
  if (input?.includeThumbnail !== undefined) payload.includeThumbnail = input.includeThumbnail
  if (input?.maxThumbnailBytes !== undefined) payload.maxThumbnailBytes = input.maxThumbnailBytes
  if (input?.maxSessions !== undefined) payload.maxSessions = input.maxSessions
  return invoke<string>('now_playing_snapshot', payload)
}

