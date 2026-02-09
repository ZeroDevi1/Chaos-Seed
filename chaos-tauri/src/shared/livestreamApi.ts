import { invoke } from '@tauri-apps/api/core'

import type { LiveManifest, StreamVariant } from './livestreamTypes'

export async function decodeManifest(input: string): Promise<LiveManifest> {
  const raw = (input || '').toString()
  return invoke<LiveManifest>('livestream_decode_manifest', { input: raw })
}

export async function resolveVariant(site: string, room_id: string, variant_id: string): Promise<StreamVariant> {
  return invoke<StreamVariant>('livestream_resolve_variant', { site, room_id, variant_id })
}

