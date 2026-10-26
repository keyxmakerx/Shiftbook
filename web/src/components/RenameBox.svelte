<script lang="ts">
  // The box an entry's name turns into while you rename it in place: Enter or clicking away
  // saves, Esc puts the old name back.
  import type { Entry } from '../lib/api';
  import { renameEntry, renaming, stopRename } from '../lib/rename.svelte';

  let { entry }: { entry: Entry } = $props();

  // svelte-ignore state_referenced_locally
  let text = $state(entry.description);
  let box: HTMLInputElement | undefined = $state();
  let done = false;

  $effect(() => {
    renaming.shown = true;
    box?.focus();
    box?.select();
  });

  function finish(save: boolean) {
    if (done) return;
    done = true;
    stopRename();
    if (save) void renameEntry(entry, text);
  }
</script>

<span class="rename">
  <input
    bind:this={box}
    bind:value={text}
    class="text-in"
    aria-label="New name"
    onblur={() => finish(true)}
    onclick={(e) => e.stopPropagation()}
    ondblclick={(e) => e.stopPropagation()}
    onkeydown={(e) => {
      e.stopPropagation();
      if (e.key === 'Enter') finish(true);
      else if (e.key === 'Escape') finish(false);
    }}
  />
  <small>Enter to save · Esc to cancel</small>
</span>

<style>
  .rename {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .rename input {
    width: 100%;
    font-weight: 600;
  }

  .rename small {
    font-size: 11px;
    color: var(--mut);
  }
</style>
