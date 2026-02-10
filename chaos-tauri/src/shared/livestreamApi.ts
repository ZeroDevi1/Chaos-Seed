import { invoke } from '@tauri-apps/api/core'

import type { LiveManifest, StreamVariant } from './livestreamTypes'

export async function decodeManifest(input: string): Promise<LiveManifest> {
  const raw = (input || '').toString()
  return invoke<LiveManifest>('livestream_decode_manifest', { input: raw })
}

export async function resolveVariant(site: string, room_id: string, variant_id: string): Promise<StreamVariant> {
  // Tauri uses serde to map JS object keys to Rust parameters; use camelCase here.
  return invoke<StreamVariant>('livestream_resolve_variant', { site, roomId: room_id, variantId: variant_id })
}
