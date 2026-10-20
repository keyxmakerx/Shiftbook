<script lang="ts">
  // The cabinet (round 8): a drawer per project plus folders you make, down the left of the
  // Desk. Drag a card or a pile onto a drawer to file it off the Desk; click a drawer to see
  // what's in it. Filing only tidies: filed things stay in Find and count on the timesheet.
  import type { CanvasDrawer } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import Icon from './Icon.svelte';

  let {
    drawers,
    hover,
    open = $bindable(false),
    peek = false,
    opened,
    onopen,
    onaddfolder,
  }: {
    drawers: CanvasDrawer[];
    /** The drawer something is being dragged over. */
    hover: string | null;
    /** The cabinet shows its drawers (folded away otherwise). */
    open?: boolean;
    /** Something is being dragged: the drawers slide out to take it, then fold away again. */
    peek?: boolean;
    /** The drawer whose contents are showing. */
    opened: string | null;
    onopen: (key: string) => void;
    onaddfolder: (name: string) => void;
  } = $props();

  let adding = $state(false);
  let name = $state('');

  const showing = $derived(open || peek);

  function add() {
    if (name.trim()) onaddfolder(name.trim());
    name = '';
    adding = false;
  }
</script>

<nav class="cabinet" class:shut={!showing} class:peek={peek && !open} aria-label="Drawers">
  <button type="button" class="head" aria-expanded={open} onclick={() => (open = !open)} title={open ? 'Fold the drawers away' : 'Show the drawers'}>
    <Icon name="archive" size={14} />
    <span>Drawers</span>
    <Icon name={open ? 'up' : 'down'} size={12} />
  </button>
  {#if showing}
    <ul>
      {#each drawers as d (d.key)}
        <li>
          <button
            type="button"
            class="drawer"
            class:folder={!d.color}
            class:hover={hover === d.key}
            class:opened={opened === d.key}
            data-drawer={d.key}
            style:--c={d.color ?? 'var(--mut)'}
            title="{d.name}: drop a card or a pile here to file it · click to see what's inside"
            onclick={() => onopen(d.key)}
          >
            <span class="handle" aria-hidden="true"></span>
            <span class="nm">{d.name}</span>
            {#if d.count}<span class="n">{d.count}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
    {#if adding}
      <input
        class="text-in add"
        bind:value={name}
        use:focusNow
        maxlength="60"
        placeholder="Folder name"
        aria-label="New folder name"
        onblur={add}
        onkeydown={(e) => {
          if (e.key === 'Enter') add();
          else if (e.key === 'Escape') {
            e.stopPropagation();
            name = '';
            adding = false;
          }
        }}
      />
    {:else}
      <button type="button" class="new" onclick={() => (adding = true)}><Icon name="plus" size={12} /> Folder</button>
    {/if}
  {/if}
</nav>

<style>
  .cabinet {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 170px;
    max-height: 100%;
    padding: 6px;
    border-radius: 11px;
    background: var(--glass);
    box-shadow: var(--shadow);
    backdrop-filter: blur(6px);
    overflow: auto;
  }

  .cabinet.shut {
    width: auto;
  }

  .cabinet.peek {
    animation: peek calc(0.22s * var(--m)) var(--spring);
  }

  @keyframes peek {
    from {
      opacity: 0.4;
      transform: translateX(-10px);
    }
  }

  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--mut);
    font: 700 10.5px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 3px 4px;
    cursor: pointer;
  }

  .head span {
    flex: 1;
    text-align: left;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .drawer {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 8px 6px 0;
    border: 1px solid var(--rule);
    border-left: 4px solid var(--c);
    border-radius: 6px;
    background: var(--paper);
    color: var(--ink);
    font: 600 12.5px var(--ui);
    cursor: pointer;
    text-align: left;
    transition:
      transform calc(0.2s * var(--m)) var(--spring),
      box-shadow 0.2s;
  }

  .drawer.folder {
    border-left-style: dashed;
  }

  .drawer:hover,
  .drawer.opened {
    box-shadow: var(--shadow);
  }

  .drawer.hover {
    transform: translateX(6px);
    box-shadow:
      0 0 0 2px var(--accent),
      var(--lift);
  }

  .handle {
    width: 14px;
    height: 4px;
    margin-left: 6px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--ink) 25%, transparent);
    flex: none;
  }

  .nm {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .n {
    font: 700 11px var(--mono);
    color: var(--mut);
  }

  .new {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 0;
    background: none;
    color: var(--accent);
    font: 600 12px var(--ui);
    padding: 4px;
    cursor: pointer;
    align-self: flex-start;
  }

  .add {
    font-size: 12.5px;
    padding: 5px 8px;
  }

  @media (prefers-reduced-motion: reduce) {
    .drawer {
      transition: none;
    }
  }
</style>
