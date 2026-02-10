export function expandHttpToHttps(url: string): string[] {
  const u = (url || '').toString().trim()
  if (!u) return []
  if (u.startsWith('http://')) return [`https://${u.slice('http://'.length)}`, u]
  return [u]
}

