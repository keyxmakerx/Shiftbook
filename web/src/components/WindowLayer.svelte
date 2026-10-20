<script lang="ts">
  // Where windows float: over the views, under menus and toasts. Minimised windows stay
  // mounted but hidden, so their drafts and titles are kept and only their bar button shows.
  import { windows, winKey } from '../lib/windows.svelte';
  import FloatWindow from './FloatWindow.svelte';
</script>

<div class="layer">
  {#each windows.list as win (winKey(win))}
    <FloatWindow {win} />
  {/each}
</div>

<style>
  .layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 40;
  }
</style>
