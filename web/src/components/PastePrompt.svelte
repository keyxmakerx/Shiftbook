<script lang="ts">
  // Paste anywhere on the Desk (round 15): tickets just pasted went to the Tickets tray;
  // this asks "Time on them so far?" where the pointer was, like the tray's own box.
  // Enter keeps 15 minutes (what a new ticket starts with), Esc closes without changing.
  import type { TrayItem } from '../lib/api';
  import { fmtMinutes } from '../lib/format';
  import { ticketTimes } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const SO_FAR = $derived(ticketTimes());

  let {
    x,
    y,
    items,
    onpick,
    onclose,
  }: {
    /** Where it shows, in board pixels. */
    x: number;
    y: number;
    items: TrayItem[];
    onpick: (minutes: number) => void;
    onclose: () => void;
  } = $props();
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    } else if (e.key === 'Enter' && !(e.target as HTMLElement).closest('input, textarea, [contenteditable="true"]')) {
      e.preventDefault();
      onpick(15);
    }
  }}
/>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="prompt paper"
  style:left="{x}px"
  style:top="{y}px"
  aria-label="Time on them so far?"
  onpointerdown={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="tks">
      {#each items as item (item.id)}<span class="tk mono">{item.ticket}</span>{/each}
    </span>
    <button type="button" class="x" aria-label="Close" onclick={onclose}><Icon name="x" size={14} /></button>
  </header>
  <b>Added to Tickets. Time on {items.length === 1 ? 'it' : 'them'} so far?</b>
  <div class="row">
    {#each SO_FAR as m (m)}
      <button type="button" class="pill mono" class:on={m === 15} onclick={() => onpick(m)}>{fmtMinutes(m)}</button>
    {/each}
  </div>
  <p class="hint">Enter for 15m · Esc to leave it</p>
</section>

<style>
  .prompt {
    position: absolute;
    z-index: 100001;
    width: 300px;
    padding: 10px 12px 12px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13.5px;
    animation: pop calc(0.2s * var(--m)) var(--spring);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.97);
    }
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }

  .tks {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .tk {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, var(--paper));
    border-radius: 5px;
    padding: 2px 6px;
  }

  .x {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 26px;
    height: 26px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .pill {
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    border-radius: 8px;
    padding: 5px 9px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .pill.on {
    background: var(--ink);
    color: var(--paper);
    border-color: var(--ink);
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--mut);
  }

  @media (prefers-reduced-motion: reduce) {
    .prompt {
      animation: none;
    }
  }
</style>
