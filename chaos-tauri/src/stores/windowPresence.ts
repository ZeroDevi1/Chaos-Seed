import { writable } from 'svelte/store'

export type KnownAuxWindowLabel = 'chat' | 'overlay'

export type WindowPresenceState = {
  chatOpen: boolean
  overlayOpen: boolean
}

const state = writable<WindowPresenceState>({
  chatOpen: false,
  overlayOpen: false
})

function setOpen(label: string, open: boolean) {
  state.update((s) => {
    if (label === 'chat') return s.chatOpen === open ? s : { ...s, chatOpen: open }
    if (label === 'overlay') return s.overlayOpen === open ? s : { ...s, overlayOpen: open }
    return s
  })
}

export const windowPresence = {
  subscribe: state.subscribe,
  setOpen
}

