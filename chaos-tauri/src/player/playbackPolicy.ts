import type { PlayerEngineKind } from './types'

export type PauseStrategy = 'pause' | 'stop'

export function choosePauseStrategy(input: { engineKind: PlayerEngineKind; isLive: boolean }): PauseStrategy {
  // For most engines we can rely on `pause()` semantics.
  //
  // For `@libmedia/avplayer` playing live FLV, `pause()` is often ineffective: the underlying worker
  // continues pulling/decoding frames, so the UI "pauses" but video keeps moving.
  // A pragmatic UX is to treat "pause" as "stop" for live avplayer sources, and "play" as reload.
  if (input.engineKind === 'avplayer' && input.isLive) return 'stop'
  return 'pause'
}

