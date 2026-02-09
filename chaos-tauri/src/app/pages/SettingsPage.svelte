<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  import { prefs } from '@/stores/prefs'
  import type { BackdropMode, OverlayMode, ThemeMode } from '@/shared/prefs'

  function readSelectValue(ev: Event): string {
    return ((ev.target as unknown as { value: string })?.value ?? '').toString()
  }

  function onThemeChange(ev: Event) {
    const v = readSelectValue(ev) as ThemeMode
    prefs.setThemeMode(v)
  }

  function onOverlayChange(ev: Event) {
    const v = readSelectValue(ev) as OverlayMode
    prefs.setOverlayMode(v)
  }

  function onBackdropChange(ev: Event) {
    const v = readSelectValue(ev) as BackdropMode
    prefs.setBackdropMode(v)
    // Best-effort: apply immediately on Windows; no-op elsewhere.
    void invoke('set_backdrop', { mode: v }).catch(() => {})
  }
</script>

<div class="page page-narrow page-settings">
  <h2 class="heading">设置</h2>
  <div class="text-secondary">提示：主题/侧边栏折叠状态/Overlay 模式/Backdrop 会自动持久化。</div>

  <!-- Use a plain container instead of fluent-card to avoid popup/listbox clipping on some WebView2 setups. -->
  <div class="card settings-card">
    <div class="settings-item">
      <div class="settings-label">主题</div>
      <fluent-select class="select" value={$prefs.themeMode} on:change={onThemeChange}>
        <fluent-option value="system">跟随系统</fluent-option>
        <fluent-option value="light">浅色主题</fluent-option>
        <fluent-option value="dark">深色主题</fluent-option>
      </fluent-select>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">Overlay 模式</div>
      <fluent-select class="select" value={$prefs.overlayMode} on:change={onOverlayChange}>
        <fluent-option value="transparent">透明（可能不稳定）</fluent-option>
        <fluent-option value="opaque">不透明（更稳，推荐）</fluent-option>
      </fluent-select>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">Backdrop（Win11）</div>
      <fluent-select class="select" value={$prefs.backdropMode} on:change={onBackdropChange}>
        <fluent-option value="mica">Mica（更像原生）</fluent-option>
        <fluent-option value="none">关闭（更稳）</fluent-option>
      </fluent-select>
    </div>

    <div class="settings-help text-muted">
      说明：透明 Overlay 在某些机器/驱动下可能更吃性能；如果出现掉帧/卡顿，改为“不透明（更稳）”。<br />
      Backdrop 仅对 Windows 生效；若出现发虚/卡顿可关闭。
    </div>
  </div>
</div>
