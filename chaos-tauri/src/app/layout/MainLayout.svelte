<script lang="ts">
  import { prefs } from '@/stores/prefs'
  import AppIcon from '@/ui/AppIcon.svelte'
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
    { label: '弹幕', path: '/danmaku', icon: 'danmaku' },
    { label: '歌词', path: '/lyrics', icon: 'subtitle' }
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

  function currentTitle(): string {
    const p = selectedPath
    const it = [...TOP_ITEMS, ...BOTTOM_ITEMS].find((x) => x.path === p)
    if (it) return it.label
    return activeDef?.path || 'Chaos Seed'
  }

  function readSelectValue(ev: Event): string {
    return ((ev.target as unknown as { value: string })?.value ?? '').toString()
  }

  function onThemeChange(ev: Event) {
    const v = readSelectValue(ev) as any
    prefs.setThemeMode(v)
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
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <fluent-button
        class="sidebar-header-btn"
        appearance="stealth"
        type="button"
        title={sidebarCollapsed ? '展开' : '折叠'}
        on:click={toggleSider}
        on:keydown={(e: KeyboardEvent) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), toggleSider())}
      >
        <span class="sidebar-icon" aria-hidden="true">≡</span>
        {#if !sidebarCollapsed}
          <span class="sidebar-title">Chaos Seed</span>
        {/if}
      </fluent-button>
    </div>

    <div class="sidebar-mid">
      <nav class="sidebar-nav" aria-label="导航">
        <fluent-tree-view class="nav-tree" aria-label="导航列表">
        {#each TOP_ITEMS as it (it.path)}
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <fluent-tree-item
            id={anchorId('top', it.path)}
            class="nav-item"
            selected={selectedPath === it.path}
            aria-current={selectedPath === it.path ? 'page' : undefined}
            on:click={() => go(it.path)}
            on:keydown={(e: KeyboardEvent) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), go(it.path))}
          >
            <span slot="start" class="nav-start" aria-hidden="true">
              <AppIcon name={it.icon} />
            </span>
            {#if !sidebarCollapsed}
              <span class="nav-label">{it.label}</span>
            {/if}
          </fluent-tree-item>
          {#if sidebarCollapsed}
            <fluent-tooltip anchor={anchorId('top', it.path)} position="right">{it.label}</fluent-tooltip>
          {/if}
        {/each}
        </fluent-tree-view>
      </nav>
    </div>

    <div class="sidebar-bottom">
      <fluent-divider class="sidebar-sep" aria-hidden="true"></fluent-divider>

      <nav class="sidebar-nav" aria-label="设置">
        <fluent-tree-view class="nav-tree" aria-label="设置列表">
        {#each BOTTOM_ITEMS as it (it.path)}
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <fluent-tree-item
            id={anchorId('bottom', it.path)}
            class="nav-item"
            selected={selectedPath === it.path}
            aria-current={selectedPath === it.path ? 'page' : undefined}
            on:click={() => go(it.path)}
            on:keydown={(e: KeyboardEvent) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), go(it.path))}
          >
            <span slot="start" class="nav-start" aria-hidden="true">
              <AppIcon name={it.icon} />
            </span>
            {#if !sidebarCollapsed}
              <span class="nav-label">{it.label}</span>
            {/if}
          </fluent-tree-item>
          {#if sidebarCollapsed}
            <fluent-tooltip anchor={anchorId('bottom', it.path)} position="right">{it.label}</fluent-tooltip>
          {/if}
        {/each}
        </fluent-tree-view>
      </nav>
    </div>
  </aside>

  <main class="main-col">
    <fluent-toolbar class="topbar" aria-label="工具栏">
      <div class="topbar-title" data-tauri-drag-region>{currentTitle()}</div>
      <div class="topbar-spacer" data-tauri-drag-region></div>
      <fluent-select class="select w-180" value={$prefs.themeMode} on:change={onThemeChange}>
        <fluent-option value="system">跟随系统</fluent-option>
        <fluent-option value="light">浅色主题</fluent-option>
        <fluent-option value="dark">深色主题</fluent-option>
      </fluent-select>
    </fluent-toolbar>

    <div class="content">
      {#each KEEP_ROUTES as r (r.path)}
        <div class="route-host keep" class:active={selectedPath === r.path} aria-hidden={selectedPath === r.path ? 'false' : 'true'}>
          <svelte:component this={r.component} />
        </div>
      {/each}

      {#if !activeDef.keepAlive}
        {#key selectedPath}
          <div class="route-host transient active">
            <svelte:component this={activeDef.component} />
          </div>
        {/key}
      {/if}
    </div>
  </main>
</div>
