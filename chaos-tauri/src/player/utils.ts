export type StreamType = 'hls' | 'flv' | 'native'

export function inferStreamType(url: string): StreamType {
  const u = (url || '').toString().toLowerCase()
  if (u.includes('.m3u8')) return 'hls'
  if (u.includes('.flv')) return 'flv'
  return 'native'
}

