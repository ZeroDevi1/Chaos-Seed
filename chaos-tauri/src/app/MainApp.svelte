<script lang="ts">
  import { onMount } from 'svelte'

  import MainLayout from './layout/MainLayout.svelte'
  import { startRouter } from './router'

  onMount(() => startRouter())
</script>

<MainLayout />

