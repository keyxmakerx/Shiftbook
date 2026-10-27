<script lang="ts">
  // Where windows float: over the views, under menus and toasts. Minimised windows stay
  // mounted but hidden, so their drafts and titles are kept and only their bar button shows.
  // The MOP notebook is one more window here, with its own tabs.
  import { windows, winKey } from '../lib/windows.svelte';
  import FloatWindow from './FloatWindow.svelte';
  import MopNotebook from './MopNotebook.svelte';
  import MopImport from './MopImport.svelte';
  import { book } from '../lib/mopbook.svelte';
</script>

<div class="layer">
  {#each windows.list as win (winKey(win))}
    <FloatWindow {win} />
  {/each}
  <!-- The MOP notebook (round 16) floats with the other windows; files dropped anywhere come in through MopImport. -->
  {#if book.loaded && book.shown}
    <MopNotebook />
  {/if}
</div>
<MopImport />

<style>
  .layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 40;
  }
</style>
