import { invoke } from '@tauri-apps/api/core'

import type { LyricsSearchResult } from './types'

export type LyricsWindowMode = 'chat' | 'overlay'

export async function openLyricsWindow(mode: LyricsWindowMode): Promise<void> {
  if (mode === 'overlay') {
    await invoke('open_lyrics_overlay_window')
    return
  }
  await invoke('open_lyrics_chat_window')
}

export async function setLyricsWindowPayload(item: LyricsSearchResult): Promise<void> {
  await invoke('lyrics_set_current', { payload: item })
}

