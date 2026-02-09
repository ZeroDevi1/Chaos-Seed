import { writable } from 'svelte/store'

import HomePage from './pages/HomePage.svelte'
import SubtitleDownloadPage from './pages/SubtitleDownloadPage.svelte'
import LiveSourcePage from './pages/LiveSourcePage.svelte'
import DanmakuPage from './pages/DanmakuPage.svelte'
import SettingsPage from './pages/SettingsPage.svelte'
import AboutPage from './pages/AboutPage.svelte'

export type RouteDef = {
  path: string
  component: any
  keepAlive?: boolean
}

export const ROUTES: RouteDef[] = [
  { path: '/', component: HomePage, keepAlive: true },
  { path: '/subtitle', component: SubtitleDownloadPage, keepAlive: true },
  { path: '/live-source', component: LiveSourcePage, keepAlive: true },
  // Keep Danmaku alive so the user can switch pages without losing input / connection / list state.
  // We explicitly control backend subscriptions based on route focus + chat window presence.
  { path: '/danmaku', component: DanmakuPage, keepAlive: true },
  { path: '/settings', component: SettingsPage, keepAlive: true },
  { path: '/about', component: AboutPage, keepAlive: true }
]

export function resolveRoute(path: string): RouteDef | null {
  const p = (path || '').trim()
  if (!p.startsWith('/')) return null
  return ROUTES.find((r) => r.path === p) ?? null
}

export function getHashPath(hash: string = typeof window !== 'undefined' ? window.location.hash : ''): string {
  const raw = (hash || '').trim()
  if (!raw) return '/'
  // Support both "#/path" and "#path" formats.
  const h = raw.startsWith('#') ? raw.slice(1) : raw
  const p = h.startsWith('/') ? h : `/${h}`
  return p === '/' ? '/' : p.replace(/\/+$/, '')
}

export function navigate(path: string) {
  const p = path === '/' ? '/' : (path || '').trim()
  if (!p.startsWith('/')) return
  window.location.hash = `#${p}`
}

export type RouteState = {
  path: string
  def: RouteDef
}

function resolveOrRoot(path: string): RouteState {
  const def = resolveRoute(path) ?? resolveRoute('/')!
  return { path: def.path, def }
}

export const routeStore = writable<RouteState>(
  resolveOrRoot(typeof window !== 'undefined' ? getHashPath(window.location.hash) : '/')
)

export function startRouter(): () => void {
  const update = () => routeStore.set(resolveOrRoot(getHashPath()))
  update()
  window.addEventListener('hashchange', update)
  return () => window.removeEventListener('hashchange', update)
}
