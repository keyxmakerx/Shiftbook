<script lang="ts">
  // "What is this box?" (round 12): after drawing a box on the Desk (Shift + drag, the Box
  // tool, or "Box these" on a selection). Pick a project, make a new one, or keep it a plain
  // labelled box that only groups cards.
  import { focusNow } from '../lib/focus';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let {
    x,
    y,
    inside,
    boxed,
    onproject,
    onnew,
    onlabel,
    oncancel,
  }: {
    /** Where it shows, in board pixels. */
    x: number;
    y: number;
    /** How many cards the box holds. */
    inside: number;
    /** Projects that already have a box (theirs moves here). */
    boxed: Set<string>;
    onproject: (projectId: string) => void;
    onnew: (name: string) => void;
    onlabel: (label: string) => void;
    oncancel: () => void;
  } = $props();

  let typing = $state<'new' | 'label' | null>(null);
  let text = $state('');

  const projects = $derived(app.projects.filter((p) => !p.archived));

  function done() {
    const t = text.trim();
    if (!t) return;
    if (typing === 'new') onnew(t);
    else onlabel(t);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      oncancel();
    }
  }}
/>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="prompt paper"
  style:left="{x}px"
  style:top="{y}px"
  aria-label="What is this box?"
  onpointerdown={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="lbl">What is this box?</span>
    <button type="button" class="x" aria-label="Never mind" onclick={oncancel}><Icon name="x" size={14} /></button>
  </header>
  {#if typing}
    <input
      class="text-in"
      bind:value={text}
      use:focusNow
      maxlength={typing === 'new' ? 80 : 60}
      placeholder={typing === 'new' ? 'New project name' : 'Name for the box'}
      aria-label={typing === 'new' ? 'New project name' : 'Box name'}
      onkeydown={(e) => {
        if (e.key === 'Enter') done();
      }}
    />
    <div class="row">
      <button type="button" class="btn primary" disabled={!text.trim()} onclick={done}>{typing === 'new' ? 'Make the project' : 'Keep the box'}</button>
      <button type="button" class="btn" onclick={() => (typing = null)}>Back</button>
    </div>
  {:else}
    <div class="choices">
      {#each projects as p (p.id)}
        <button type="button" class="pill" style:--c={p.color ?? 'var(--accent)'} title={boxed.has(p.id) ? 'Its box moves here' : ''} onclick={() => onproject(p.id)}>
          <span class="sw"></span>{p.name}{#if boxed.has(p.id)}<small> · move</small>{/if}
        </button>
      {/each}
    </div>
    <div class="row">
      <button type="button" class="pill new" onclick={() => (typing = 'new')}><Icon name="plus" size={12} /> New project…</button>
      <button type="button" class="pill" onclick={() => (typing = 'label')}>Just a label</button>
    </div>
    <p class="hint">
      {inside === 0 ? 'No cards inside yet.' : `${inside} card${inside === 1 ? '' : 's'} inside will get the project you pick (with Undo).`}
    </p>
  {/if}
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
    align-items: center;
  }

  .lbl {
    flex: 1;
    font: 700 10px var(--ui);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--mut);
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

  .choices,
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .choices {
    max-height: 180px;
    overflow: auto;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    border-radius: 8px;
    padding: 5px 9px;
    font: 600 12.5px var(--ui);
    cursor: pointer;
  }

  .pill:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .pill small {
    color: var(--mut);
    font-weight: 500;
  }

  .pill.new {
    color: var(--accent);
  }

  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    background: var(--c);
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
