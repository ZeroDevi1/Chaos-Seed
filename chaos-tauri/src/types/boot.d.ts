declare global {
  interface Window {
    __CHAOS_SEED_BOOT?:
      | {
          view?: 'main' | 'chat' | 'overlay'
          overlayOpaque?: boolean
          label?: 'main' | 'chat' | 'overlay' | string
          build?: string
        }
      | undefined
  }
}

export {}
