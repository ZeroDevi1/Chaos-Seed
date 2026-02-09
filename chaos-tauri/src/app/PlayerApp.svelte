<script lang="ts">
  import PlayerWindow from './windows/PlayerWindow.svelte'
</script>

<div class="player-app">
  <PlayerWindow />
</div>

<style>
  /* Ensure the player view participates in the same 100%-height layout as other boot views. */
  .player-app {
    width: 100%;
    height: 100%;
  }
</style>
