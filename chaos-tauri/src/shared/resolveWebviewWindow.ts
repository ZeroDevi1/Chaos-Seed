import { getCurrentWebviewWindow, WebviewWindow } from '@tauri-apps/api/webviewWindow'

export async function resolveWebviewWindow(label: string): Promise<ReturnType<typeof getCurrentWebviewWindow> | null> {
  try {
    return getCurrentWebviewWindow()
  } catch {
    // ignore
  }
  try {
    return await WebviewWindow.getByLabel(label)
  } catch {
    return null
  }
}

