import { invoke } from '@tauri-apps/api/core'

import type { ThunderSubtitleItem } from './types'

export async function subtitleSearch(input: {
  query: string
  minScore?: number | null
  lang?: string | null
  limit?: number | null
}): Promise<ThunderSubtitleItem[]> {
  const query = (input.query ?? '').toString()
  const payload: Record<string, unknown> = { query }
  if (input.minScore !== undefined && input.minScore !== null) payload.minScore = input.minScore
  if (input.lang !== undefined) payload.lang = input.lang
  if (input.limit !== undefined && input.limit !== null) payload.limit = input.limit
  return invoke<ThunderSubtitleItem[]>('subtitle_search', payload)
}

export async function subtitleDownload(input: {
  item: ThunderSubtitleItem
  outDir: string
  overwrite?: boolean | null
}): Promise<string> {
  const payload: Record<string, unknown> = {
    item: input.item,
    outDir: input.outDir
  }
  if (input.overwrite !== undefined) payload.overwrite = input.overwrite
  return invoke<string>('subtitle_download', payload)
}

