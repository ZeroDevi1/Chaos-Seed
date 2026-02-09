import { AvPlayerEngine } from './engines/avplayer'
import { HlsEngine } from './engines/hls'
import { NativeEngine } from './engines/native'
import type { PlayerEngine, PlayerEngineKind } from './types'

export function createEngine(kind: PlayerEngineKind): PlayerEngine {
  switch (kind) {
    case 'hls':
      return new HlsEngine()
    case 'avplayer':
      return new AvPlayerEngine()
    case 'native':
    default:
      return new NativeEngine()
  }
}

