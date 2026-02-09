import {
  accentBaseColor,
  baseLayerLuminance,
  fluentButton,
  fluentCard,
  fluentNumberField,
  fluentOption,
  fluentSelect,
  fluentTextField,
  provideFluentDesignSystem,
  StandardLuminance,
  SwatchRGB
} from '@fluentui/web-components'

export type ResolvedTheme = 'light' | 'dark'

// Match our CSS accent (WeChat-like green): #07C160
const ACCENT = SwatchRGB.create(7, 193, 96)

let registered = false
const appliedCache = new WeakMap<HTMLElement, { lum?: number; accent?: unknown }>()

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
    fluentOption()
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
  if (prev.accent !== ACCENT) {
    accentBaseColor.setValueFor(host, ACCENT)
    prev.accent = ACCENT
  }

  appliedCache.set(host, prev)
}
