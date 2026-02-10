import { describe, expect, it } from 'vitest'

import { expandHttpToHttps } from './urlNormalize'

describe('urlNormalize.expandHttpToHttps', () => {
  it('prepends https candidate for http urls', () => {
    expect(expandHttpToHttps('http://a/b')).toEqual(['https://a/b', 'http://a/b'])
  })

  it('keeps https urls as-is', () => {
    expect(expandHttpToHttps('https://a/b')).toEqual(['https://a/b'])
  })

  it('ignores empty strings', () => {
    expect(expandHttpToHttps('')).toEqual([])
    expect(expandHttpToHttps('   ')).toEqual([])
  })
})

