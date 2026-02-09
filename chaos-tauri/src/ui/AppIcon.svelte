<script lang="ts">
  import { ICONS, type AppIconName } from './icons'

  export let name: AppIconName
  export let size: number = 20
  export let title: string = ''

  $: svg = ICONS[name]
  $: titleAttr = title?.trim() ? title : undefined
</script>

<span
  class="app-icon"
  style:width="{size}px"
  style:height="{size}px"
  title={titleAttr}
  aria-hidden="true"
>
  {@html svg}
</span>

<style>
  .app-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: inherit;
  }

  .app-icon :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  .app-icon :global(path) {
    fill: currentColor;
  }
</style>

