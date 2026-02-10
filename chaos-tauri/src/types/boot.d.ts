declare global {
  interface Window {
    __CHAOS_SEED_BOOT?:
      | {
          view?: 'main' | 'chat' | 'overlay' | 'player' | 'lyrics_chat' | 'lyrics_overlay' | 'lyrics_dock' | 'lyrics_float'
          overlayOpaque?: boolean
          label?: 'main' | 'chat' | 'overlay' | 'player' | 'lyrics_chat' | 'lyrics_overlay' | 'lyrics_dock' | 'lyrics_float' | string
          build?: string
          player?: unknown
        }
      | undefined
  }
}

export {}
