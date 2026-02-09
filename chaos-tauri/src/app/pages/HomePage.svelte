<script lang="ts">
  import { navigate } from '../router'
</script>

<div class="page page-narrow">
  <h1 class="title">Chaos Seed</h1>

  <div class="text-secondary">在迅雷字幕接口中搜索并下载字幕（Rust + Tauri）。</div>
  <div class="text-muted">推荐流程：输入关键词 -&gt; 搜索 -&gt; 点击单条下载 -&gt; 选择目录。</div>

  <fluent-card class="app-card">
    <div class="card-pad">
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <fluent-button appearance="accent" on:click={() => navigate('/subtitle')}>前往字幕下载</fluent-button>
    </div>
  </fluent-card>
</div>
