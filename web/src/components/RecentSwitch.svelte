<script lang="ts">
  // "Jump back" (round 21): hold Ctrl and press Tab to see what you had open lately; Tab
  // again moves down the list, and letting go of Ctrl opens the one that's marked. Opened
  // from Find it waits for a pick instead (Enter, or a click).
  import { menuOpen } from '../lib/menu.svelte';
  import { closeRecent, jump, jumpList, moveRecent, openRecent, recent } from '../lib/recent.svelte';
  import { app } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  const list = $derived(recent.open ? jumpList() : []);

  function pick() {
    const item = list[recent.sel];
    if (item) void jump(item);
    else closeRecent();
  }

  function down(e: KeyboardEvent) {
    if (e.key === 'Tab' && e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      if (recent.open) moveRecent(e.shiftKey ? -1 : 1);
      else if (!menuOpen() && !app.captureOpen && !openRecent(true, e.shiftKey)) toast('Nothing to jump back to yet.');
      return;
    }
    if (!recent.open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeRecent();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      pick();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      moveRecent(e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey) ? -1 : 1);
    }
  }

  function up(e: KeyboardEvent) {
    if (recent.open && !recent.sticky && e.key === 'Control') pick();
  }
</script>

<svelte:window onkeydowncapture={down} onkeyup={up} onblur={closeRecent} />

{#if recent.open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={closeRecent}></div>
  <div class="box paper" role="dialog" aria-modal="true" aria-labelledby="recent-title">
    <h2 id="recent-title">Jump back to</h2>
    <ul role="listbox" aria-label="What you had open lately">
      {#each list as r, i (r.key)}
        <li role="option" aria-selected={i === recent.sel}>
          <button
            type="button"
            class:on={i === recent.sel}
            onclick={() => void jump(r)}
            onpointerenter={() => (recent.sel = i)}
          >
            <span class="t">{r.title}</span>
            <span class="w">{r.word}</span>
          </button>
        </li>
      {/each}
    </ul>
    <p class="hint">
      {recent.sticky ? '↑ ↓ to move, Enter to open it, Esc to stay put.' : 'Tab again to move down. Let go of Ctrl to open it.'}
    </p>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 84;
    background: color-mix(in srgb, var(--ink) 14%, transparent);
  }

  .box {
    position: fixed;
    z-index: 85;
    left: 50%;
    top: 22%;
    transform: translateX(-50%);
    width: min(420px, calc(100% - 32px));
    padding: 14px 14px 12px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h2 {
    margin: 0 4px;
    font: 700 17px var(--display);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  li button {
    width: 100%;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  li button.on {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .t {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .w {
    flex: none;
    font-size: 12px;
    color: var(--mut);
  }

  .hint {
    margin: 0 4px;
    font-size: 12.5px;
    color: var(--mut);
  }
</style>
