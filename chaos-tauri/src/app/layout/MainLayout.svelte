<script lang="ts">
  import AppIcon from '@/ui/AppIcon.svelte'
  import { prefs } from '@/stores/prefs'
  import type { AppIconName } from '@/ui/icons'

  import { ROUTES, navigate, routeStore, type RouteDef } from '../router'

  type NavItem = {
    label: string
    path: string
    icon: AppIconName
  }

  const TOP_ITEMS: NavItem[] = [
    { label: '首页', path: '/', icon: 'home' },
    { label: '字幕下载', path: '/subtitle', icon: 'subtitle' },
    { label: '直播源', path: '/live-source', icon: 'live' },
    { label: '弹幕', path: '/danmaku', icon: 'danmaku' }
  ]

  const BOTTOM_ITEMS: NavItem[] = [
    { label: '设置', path: '/settings', icon: 'settings' },
    { label: '关于', path: '/about', icon: 'about' }
  ]

  const KEEP_ROUTES: RouteDef[] = ROUTES.filter((r) => r.keepAlive)

  $: selectedPath = $routeStore.path
  $: activeDef = $routeStore.def
  $: sidebarCollapsed = $prefs.sidebarCollapsed

  function anchorId(kind: 'top' | 'bottom', path: string) {
    const safe = (path === '/' ? 'root' : path.slice(1)).replaceAll('/', '_') || 'root'
    return `nav-${kind}-${safe}`
  }

  function toggleSider() {
    // NOTE(Svelte 5): avoid accessing `$prefs` inside event handlers; read the reactive value instead.
    prefs.setSidebarCollapsed(!sidebarCollapsed)
  }

  function go(path: string) {
    if (path === selectedPath) return
    navigate(path)
  }
</script>

<div class="app-root">
  <aside class="sidebar" class:collapsed={sidebarCollapsed}>
    <div class="sidebar-header">
      <button
        class="sidebar-header-btn"
        type="button"
        title={sidebarCollapsed ? '展开' : '折叠'}
        on:click={toggleSider}
      >
        <span class="sidebar-icon" aria-hidden="true">≡</span>
        <span class="sidebar-title">Chaos Seed</span>
      </button>
    </div>

    <div class="sidebar-mid">
      <nav class="sidebar-nav" aria-label="导航">
        {#each TOP_ITEMS as it (it.path)}
          <button
            id={anchorId('top', it.path)}
            class="sidebar-item"
            class:selected={selectedPath === it.path}
            type="button"
            title={sidebarCollapsed ? it.label : undefined}
            aria-current={selectedPath === it.path ? 'page' : undefined}
            on:click={() => go(it.path)}
          >
            <span class="sidebar-indicator" aria-hidden="true"></span>
            <span class="sidebar-icon" aria-hidden="true">
              <AppIcon name={it.icon} />
            </span>
            <span class="sidebar-text">{it.label}</span>
          </button>
        {/each}
      </nav>
    </div>

    <div class="sidebar-bottom">
      <div class="sidebar-sep" aria-hidden="true"></div>

      <nav class="sidebar-nav" aria-label="设置">
        {#each BOTTOM_ITEMS as it (it.path)}
          <button
            id={anchorId('bottom', it.path)}
            class="sidebar-item"
            class:selected={selectedPath === it.path}
            type="button"
            title={sidebarCollapsed ? it.label : undefined}
            aria-current={selectedPath === it.path ? 'page' : undefined}
            on:click={() => go(it.path)}
          >
            <span class="sidebar-indicator" aria-hidden="true"></span>
            <span class="sidebar-icon" aria-hidden="true">
              <AppIcon name={it.icon} />
            </span>
            <span class="sidebar-text">{it.label}</span>
          </button>
        {/each}
      </nav>
    </div>
  </aside>

  <main class="main-col">
    <div class="content">
      {#each KEEP_ROUTES as r (r.path)}
        <div class="route-host" style:display={selectedPath === r.path ? 'flex' : 'none'}>
          <svelte:component this={r.component} />
        </div>
      {/each}

      {#if !activeDef.keepAlive}
        {#key selectedPath}
          <div class="route-host">
            <svelte:component this={activeDef.component} />
          </div>
        {/key}
      {/if}
    </div>
  </main>
</div>
