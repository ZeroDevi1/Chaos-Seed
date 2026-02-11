import {
  accentBaseColor,
  baseLayerLuminance,
  fluentButton,
  fluentCard,
  fluentDivider,
  fluentMenu,
  fluentMenuItem,
  fluentNumberField,
  fluentOption,
  fluentSelect,
  fluentSkeleton,
  fluentTextField,
  fluentToolbar,
  fluentTooltip,
  fluentTreeItem,
  fluentTreeView,
  provideFluentDesignSystem,
  StandardLuminance,
  SwatchRGB
} from '@fluentui/web-components'

export type ResolvedTheme = 'light' | 'dark'

// Fallback accent (keeps a stable look when the system accent is unavailable).
const FALLBACK_ACCENT = SwatchRGB.create(7, 193, 96) // #07C160

let registered = false
const appliedCache = new WeakMap<HTMLElement, { lum?: number; accent?: unknown }>()

type RgbReply = { r: number; g: number; b: number }

function clampByte(n: number): number {
  if (!Number.isFinite(n)) return 0
  return Math.max(0, Math.min(255, Math.round(n)))
}

let accentSwatch = FALLBACK_ACCENT
let accentInitPromise: Promise<void> | null = null

function ensureRegistered() {
  if (registered) return
  registered = true
  // Register only the components we use to keep startup and runtime overhead low.
  provideFluentDesignSystem().register(
    fluentButton(),
    fluentCard(),
    fluentTextField(),
    fluentNumberField(),
    fluentSelect(),
    fluentOption(),
    fluentDivider(),
    fluentTreeView(),
    fluentTreeItem(),
    fluentTooltip(),
    fluentToolbar(),
    fluentMenu(),
    fluentMenuItem(),
    fluentSkeleton()
  )
}

// Register components at import time so the initial render upgrades immediately.
ensureRegistered()

function getTokenHost(): HTMLElement {
  // Fluent examples typically apply tokens to the document body.
  // Using a stable host avoids churn from app DOM changes.
  if (document.body) return document.body
  return document.documentElement
}

/**
 * Sync Fluent design tokens to our resolved theme.
 *
 * NOTE: This controls Fluent Web Components rendering; our app shell colors are still driven by
 * `html[data-theme=...]` CSS variables in `style.css`.
 */
export function applyFluentTokens(resolved: ResolvedTheme) {
  // Emergency kill switch: if FAST design tokens cause recursion/freezes in a given webview/engine,
  // allow disabling token writes without removing Fluent components.
  try {
    if (localStorage.getItem('chaos_seed_disable_fluent_tokens') === '1') return
  } catch {
    // ignore
  }

  ensureRegistered()
  const host = getTokenHost()
  const nextLum = resolved === 'dark' ? StandardLuminance.DarkMode : StandardLuminance.LightMode

  const prev = appliedCache.get(host) ?? {}

  // Guard against redundant writes; FAST design tokens can be expensive and (in some engines)
  // can trigger deep synchronous dependency resolution.
  if (prev.lum !== nextLum) {
    baseLayerLuminance.setValueFor(host, nextLum)
    prev.lum = nextLum
  }
  if (prev.accent !== accentSwatch) {
    accentBaseColor.setValueFor(host, accentSwatch)
    prev.accent = accentSwatch
  }

  appliedCache.set(host, prev)
}

/**
 * Best-effort: read Windows system accent and sync it to Fluent tokens.
 *
 * This is safe to call on any platform; unsupported platforms no-op.
 */
export function initSystemAccent(): Promise<void> {
  if (accentInitPromise) return accentInitPromise
  accentInitPromise = (async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const rgb = await invoke<RgbReply>('system_accent_rgb')
      const r = clampByte(rgb?.r)
      const g = clampByte(rgb?.g)
      const b = clampByte(rgb?.b)
      accentSwatch = SwatchRGB.create(r, g, b)
    } catch {
      // ignore - keep fallback accent
    }
  })()
  return accentInitPromise
}
