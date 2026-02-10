import { invoke } from '@tauri-apps/api/core'

import type { LyricsSearchResult } from './types'

export async function lyricsSearch(input: {
  title: string
  album?: string | null
  artist?: string | null
  durationMs?: number | null
  limit?: number | null
  strictMatch?: boolean | null
  servicesCsv?: string | null
  timeoutMs?: number | null
}): Promise<LyricsSearchResult[]> {
  const title = (input.title ?? '').toString()
  const payload: Record<string, unknown> = { title }
  if (input.album !== undefined) payload.album = input.album
  if (input.artist !== undefined) payload.artist = input.artist
  if (input.durationMs !== undefined && input.durationMs !== null) payload.durationMs = input.durationMs
  if (input.limit !== undefined && input.limit !== null) payload.limit = input.limit
  if (input.strictMatch !== undefined && input.strictMatch !== null) payload.strictMatch = input.strictMatch
  if (input.servicesCsv !== undefined) payload.servicesCsv = input.servicesCsv
  if (input.timeoutMs !== undefined && input.timeoutMs !== null) payload.timeoutMs = input.timeoutMs
  return invoke<LyricsSearchResult[]>('lyrics_search', payload)
}

