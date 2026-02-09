declare global {
  interface Window {
    __CHAOS_SEED_BOOT?:
      | {
          view?: 'main' | 'chat' | 'overlay' | 'player'
          overlayOpaque?: boolean
          label?: 'main' | 'chat' | 'overlay' | 'player' | string
          build?: string
          player?: unknown
        }
      | undefined
  }
}

export {}
