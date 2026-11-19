<script lang="ts">
  // The small Find bar for the open notepad or MOP (round 21): Ctrl+F opens it, Enter goes to
  // the next match and Shift+Enter to the one before, Esc closes it. The match is marked where
  // it is; see lib/finder.svelte.ts.
  import { countWords } from '../lib/findIn';
  import { closeFind, finder, setFindQuery, stepFind } from '../lib/finder.svelte';
  import Icon from './Icon.svelte';

  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    void finder.serial;
    if (!finder.open) return;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  });
</script>

{#if finder.open}
  {#each finder.boxes as b, i (i)}
    <div class="hit" style:left="{b.x}px" style:top="{b.y}px" style:width="{b.w}px" style:height="{b.h}px"></div>
  {/each}
  <div class="bar paper" role="search" style:left="{finder.at.x}px" style:top="{finder.at.y}px">
    <Icon name="search" size={13} />
    <input
      bind:this={input}
      type="text"
      value={finder.query}
      placeholder="Find in here"
      aria-label="Find in this notepad"
      autocomplete="off"
      spellcheck="false"
      oninput={(e) => setFindQuery(e.currentTarget.value)}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          stepFind(e.shiftKey ? -1 : 1);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          closeFind();
        }
      }}
    />
    <span class="n" class:none={!!finder.query && finder.total === 0} aria-live="polite">{countWords(finder.query, finder.index, finder.total)}</span>
    <button type="button" aria-label="The match before" title="The match before (Shift+Enter)" disabled={finder.total < 2} onclick={() => stepFind(-1)}>
      <Icon name="up" size={13} />
    </button>
    <button type="button" aria-label="The next match" title="The next match (Enter)" disabled={finder.total < 2} onclick={() => stepFind(1)}>
      <Icon name="down" size={13} />
    </button>
    <button type="button" aria-label="Close Find" title="Close (Esc)" onclick={() => closeFind()}><Icon name="x" size={13} /></button>
  </div>
{/if}

<style>
  .hit {
    position: fixed;
    z-index: 44;
    pointer-events: none;
    border-radius: 3px;
    background: color-mix(in srgb, #f5c400 45%, transparent);
    outline: 2px solid color-mix(in srgb, #d99a00 80%, transparent);
  }

  .bar {
    position: fixed;
    z-index: 45;
    width: 300px;
    height: 36px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 9px;
    border-radius: 10px;
    border: 1px solid var(--rule);
    box-shadow: var(--lift);
    color: var(--mut);
  }

  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    font: 13.5px var(--ui);
    color: var(--ink);
  }

  .n {
    flex: none;
    font-size: 11.5px;
    white-space: nowrap;
  }

  .n.none {
    color: var(--bad);
  }

  button {
    flex: none;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--mut);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
