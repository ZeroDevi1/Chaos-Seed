import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/webviewWindow', () => {
  return {
    getCurrentWebviewWindow: vi.fn(),
    WebviewWindow: {
      getByLabel: vi.fn()
    }
  }
})

import { getCurrentWebviewWindow, WebviewWindow } from '@tauri-apps/api/webviewWindow'

import { resolveWebviewWindow } from './resolveWebviewWindow'

describe('resolveWebviewWindow', () => {
  beforeEach(() => {
    vi.mocked(getCurrentWebviewWindow).mockReset()
    vi.mocked(WebviewWindow.getByLabel).mockReset()
  })

  it('prefers getCurrentWebviewWindow when available', async () => {
    const w = { label: 'lyrics_dock' } as any
    vi.mocked(getCurrentWebviewWindow).mockReturnValueOnce(w)

    await expect(resolveWebviewWindow('lyrics_dock')).resolves.toBe(w)
    expect(WebviewWindow.getByLabel).not.toHaveBeenCalled()
  })

  it('falls back to getByLabel when getCurrentWebviewWindow throws', async () => {
    const w = { label: 'lyrics_float' } as any
    vi.mocked(getCurrentWebviewWindow).mockImplementationOnce(() => {
      throw new Error('not in tauri context')
    })
    vi.mocked(WebviewWindow.getByLabel).mockResolvedValueOnce(w)

    await expect(resolveWebviewWindow('lyrics_float')).resolves.toBe(w)
    expect(WebviewWindow.getByLabel).toHaveBeenCalledWith('lyrics_float')
  })

  it('returns null when neither method works', async () => {
    vi.mocked(getCurrentWebviewWindow).mockImplementationOnce(() => {
      throw new Error('no window')
    })
    vi.mocked(WebviewWindow.getByLabel).mockRejectedValueOnce(new Error('missing'))

    await expect(resolveWebviewWindow('nope')).resolves.toBeNull()
  })
})

