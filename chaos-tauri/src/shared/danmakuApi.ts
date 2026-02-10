import { invoke } from '@tauri-apps/api/core'

export type DanmakuImageReply = {
  mime: string
  bytes: number[]
}

export async function fetchDanmakuImage(input: {
  url: string
  site?: string | null
  roomId?: string | null
}): Promise<DanmakuImageReply> {
  const payload: Record<string, unknown> = { url: input.url }
  if (input.site !== undefined) payload.site = input.site
  if (input.roomId !== undefined) payload.roomId = input.roomId
  return invoke<DanmakuImageReply>('danmaku_fetch_image', payload)
}

