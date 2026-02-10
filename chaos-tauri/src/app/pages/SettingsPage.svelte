<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'

  import { prefs } from '@/stores/prefs'
  import type { BackdropMode, OverlayMode, ThemeMode } from '@/shared/prefs'

  type LyricsEffects = {
    background_effect: 'none' | 'fluid'
    layout_effect: 'none' | 'fan3d'
    particle_effect: 'none' | 'snow'
  }

  type LyricsSettings = {
    lyrics_detection_enabled: boolean
    auto_hide_on_pause: boolean
    auto_hide_delay_ms: number
    matching_threshold: number
    timeout_ms: number
    effects: LyricsEffects
  }

  let lyrics: LyricsSettings | null = null
  let lyricsBusy = false

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

  async function refreshLyrics() {
    lyricsBusy = true
    try {
      const s = (await invoke('lyrics_settings_get')) as any
      lyrics = {
        lyrics_detection_enabled: !!s?.lyrics_detection_enabled,
        auto_hide_on_pause: s?.auto_hide_on_pause ?? true,
        auto_hide_delay_ms: s?.auto_hide_delay_ms ?? 800,
        matching_threshold: s?.matching_threshold ?? 40,
        timeout_ms: s?.timeout_ms ?? 8000,
        effects: {
          background_effect: (s?.effects?.background_effect ?? 'none') as any,
          layout_effect: (s?.effects?.layout_effect ?? 'none') as any,
          particle_effect: (s?.effects?.particle_effect ?? 'none') as any
        }
      }
    } catch {
      lyrics = null
    } finally {
      lyricsBusy = false
    }
  }

  async function patchLyrics(p: any) {
    if (lyricsBusy) return
    lyricsBusy = true
    try {
      const out = (await invoke('lyrics_settings_set', { partial: p })) as any
      lyrics = {
        lyrics_detection_enabled: !!out?.lyrics_detection_enabled,
        auto_hide_on_pause: out?.auto_hide_on_pause ?? true,
        auto_hide_delay_ms: out?.auto_hide_delay_ms ?? 800,
        matching_threshold: out?.matching_threshold ?? 40,
        timeout_ms: out?.timeout_ms ?? 8000,
        effects: {
          background_effect: (out?.effects?.background_effect ?? 'none') as any,
          layout_effect: (out?.effects?.layout_effect ?? 'none') as any,
          particle_effect: (out?.effects?.particle_effect ?? 'none') as any
        }
      }
    } catch {
      // ignore
    } finally {
      lyricsBusy = false
    }
  }

  onMount(() => {
    void refreshLyrics()
  })
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

  <div class="card settings-card" style="margin-top: 12px">
    <div class="settings-item">
      <div class="settings-label">歌词检测</div>
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <fluent-button
        class="w-180"
        appearance={lyrics?.lyrics_detection_enabled ? 'accent' : 'outline'}
        disabled={lyricsBusy || !lyrics}
        on:click={() => patchLyrics({ lyrics_detection_enabled: !lyrics?.lyrics_detection_enabled })}
      >
        {lyrics?.lyrics_detection_enabled ? '已开启' : '已关闭'}
      </fluent-button>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">暂停自动隐藏</div>
      <fluent-select
        class="select"
        value={lyrics?.auto_hide_on_pause ? 'on' : 'off'}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ auto_hide_on_pause: readSelectValue(ev) === 'on' })}
      >
        <fluent-option value="on">开启</fluent-option>
        <fluent-option value="off">关闭</fluent-option>
      </fluent-select>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">隐藏延迟 (ms)</div>
      <input
        class="input w-220"
        type="number"
        min="50"
        max="10000"
        step="50"
        value={lyrics?.auto_hide_delay_ms ?? 800}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ auto_hide_delay_ms: Number((ev.target as any).value || 800) })}
      />
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">匹配阈值</div>
      <input
        class="input w-220"
        type="number"
        min="0"
        max="100"
        step="1"
        value={lyrics?.matching_threshold ?? 40}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ matching_threshold: Number((ev.target as any).value || 40) })}
      />
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">背景特效</div>
      <fluent-select
        class="select"
        value={lyrics?.effects.background_effect ?? 'none'}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ effects: { ...(lyrics?.effects || {}), background_effect: readSelectValue(ev) } })}
      >
        <fluent-option value="none">无</fluent-option>
        <fluent-option value="fluid">流体</fluent-option>
      </fluent-select>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">布局特效</div>
      <fluent-select
        class="select"
        value={lyrics?.effects.layout_effect ?? 'none'}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ effects: { ...(lyrics?.effects || {}), layout_effect: readSelectValue(ev) } })}
      >
        <fluent-option value="none">无</fluent-option>
        <fluent-option value="fan3d">扇形 3D</fluent-option>
      </fluent-select>
    </div>
    <div class="divider"></div>

    <div class="settings-item">
      <div class="settings-label">粒子特效</div>
      <fluent-select
        class="select"
        value={lyrics?.effects.particle_effect ?? 'none'}
        disabled={lyricsBusy || !lyrics}
        on:change={(ev: Event) => patchLyrics({ effects: { ...(lyrics?.effects || {}), particle_effect: readSelectValue(ev) } })}
      >
        <fluent-option value="none">无</fluent-option>
        <fluent-option value="snow">雪花</fluent-option>
      </fluent-select>
    </div>

    <div class="settings-help text-muted">
      说明：歌词检测开启后，将自动从系统 Now Playing 获取当前曲目并按顺序搜索 QQ/网易云/LRCLIB。<br />
      若感觉吃性能，可关闭背景/粒子特效。
    </div>
  </div>
</div>
