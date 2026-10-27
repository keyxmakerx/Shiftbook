<script lang="ts">
  // What's in a drawer: everything filed there, newest first. Drop a card or a pile on the
  // open panel to file it here too (round 14), as on the drawer's name in the cabinet. Drag
  // its title bar to move it out of the way. Put back brings a card onto
  // the Desk where you're looking; Open opens it. A folder can be renamed or removed.
  import { api } from '../lib/api';
  import type { CanvasCard, CanvasDrawer } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { fmtMinutes } from '../lib/format';
  import { KIND_LABEL } from '../lib/notes';
  import { untrack } from 'svelte';
  import { openBoard } from '../lib/boards.svelte';
  import { app, attempt, openNote } from '../lib/store.svelte';
  import { openWindow } from '../lib/windows.svelte';
  import { dragButton } from '../lib/rightDrag';
  import Icon from './Icon.svelte';

  let {
    drawer,
    putBack,
    onclose,
    onchanged,
    onrename,
    onremove,
    dropping = false,
    shift = $bindable({ x: 0, y: 0 }),
  }: {
    drawer: CanvasDrawer;
    /** Something is being dragged over it (dropping files it here). */
    dropping?: boolean;
    /** How far it was dragged from its usual spot, in pixels. */
    shift?: { x: number; y: number };
    /** Where a card put back lands (canvas units). */
    putBack: () => { x: number; y: number };
    onclose: () => void;
    /** Something moved in or out; the Desk reloads. */
    onchanged: () => void;
    onrename: (name: string) => void;
    onremove: () => void;
  } = $props();

  let cards = $state<CanvasCard[] | null>(null);
  let renaming = $state(false);
  let draft = $state('');
  const folder = $derived(drawer.key.startsWith('folder:'));

  async function load() {
    cards = (await attempt(() => api.canvasDrawer(drawer.key))) ?? [];
  }

  // Loads on opening, and again when the drawer's count or anything else changes.
  $effect(() => {
    void drawer.count;
    void app.changed;
    untrack(() => void load());
  });

  function title(c: CanvasCard): { kind: string; text: string } {
    if (c.entry) return { kind: `Entry · ${c.entry.hours} h`, text: c.entry.description || 'Untitled' };
    if (c.tray) return { kind: `Ticket · ${fmtMinutes(c.tray.item.minutes)}`, text: c.tray.item.ticket };
    if (c.note) return { kind: `${KIND_LABEL[c.note.kind]} note`, text: c.note.title };
    return { kind: 'Inbox scrap', text: c.inbox?.text ?? '' };
  }

  async function back(c: CanvasCard) {
    const at = putBack();
    if (await attempt(() => api.canvasUnfile({ kind: c.kind, id: c.id }, at.x, at.y).then(() => true))) onchanged();
  }

  function open(c: CanvasCard) {
    if (c.entry) app.drawer = c.entry.id;
    else if (c.note) openNote(c.note.id);
    else if (c.tray) openWindow('tray', c.tray.item.id);
  }

  /** Drag the title bar to move the panel (buttons and the rename box keep their clicks). */
  function moveStart(e: PointerEvent) {
    if (!dragButton(e) || (e.target as HTMLElement).closest('button, input')) return;
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    const start = { x: e.clientX - shift.x, y: e.clientY - shift.y };
    target.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      // Keep the title bar on screen.
      const x = Math.max(-180, Math.min(window.innerWidth - 300, m.clientX - start.x));
      const y = Math.max(-10, Math.min(window.innerHeight - 160, m.clientY - start.y));
      shift = { x, y };
    };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape' && !renaming) {
      e.stopPropagation();
      onclose();
    }
  }}
/>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="panel paper"
  class:dropping
  data-drawer={drawer.key}
  style:--c={drawer.color ?? 'var(--mut)'}
  style:transform="translate({shift.x}px, {shift.y}px)"
  aria-label="{drawer.name} drawer"
  onpointerdown={(e) => e.stopPropagation()}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header title="Drag to move" onpointerdown={moveStart}>
    {#if renaming}
      <input
        class="text-in"
        bind:value={draft}
        use:focusNow
        maxlength="60"
        aria-label="Folder name"
        onblur={() => {
          renaming = false;
          if (draft.trim() && draft.trim() !== drawer.name) onrename(draft.trim());
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
          else if (e.key === 'Escape') {
            e.stopPropagation();
            renaming = false;
          }
        }}
      />
    {:else}
      <h3>{drawer.name}</h3>
    {/if}
    {#if folder && !renaming}
      <button
        type="button"
        class="ib"
        title="Rename the folder"
        aria-label="Rename the folder"
        onclick={() => {
          draft = drawer.name;
          renaming = true;
        }}><Icon name="edit" size={14} /></button
      >
      <button type="button" class="ib" title="Remove the folder (its cards go back on the Desk)" aria-label="Remove the folder" onclick={onremove}>
        <Icon name="trash" size={14} />
      </button>
    {/if}
    {#if !renaming}
      <button type="button" class="ib" title="Open full screen" aria-label="Open {drawer.name} full screen" onclick={() => openBoard(drawer.key)}>
        <Icon name="expand" size={14} />
      </button>
    {/if}
    <button type="button" class="ib" aria-label="Close" onclick={onclose}><Icon name="x" size={14} /></button>
  </header>
  {#if cards === null}
    <p class="empty">Opening…</p>
  {:else if cards.length === 0}
    <p class="empty">Nothing filed here yet. Drag a card or a pile here (or onto the drawer's name) to file it.</p>
  {:else}
    <ul>
      {#each cards as c (c.kind + c.id)}
        {@const t = title(c)}
        <li>
          <span class="txt"><small>{t.kind}</small><b>{t.text}</b></span>
          <button type="button" class="btn" onclick={() => open(c)}>Open</button>
          <button type="button" class="btn" onclick={() => void back(c)}>Put back</button>
        </li>
      {/each}
    </ul>
  {/if}
  <p class="foot">Filed things still show in Find and count on the timesheet.</p>
</section>

<style>
  .panel {
    width: 320px;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 12px;
    border-top: 4px solid var(--c);
    box-shadow: var(--lift);
    overflow: auto;
    animation: slide calc(0.25s * var(--m)) var(--spring);
    /* Dragging over it mustn't select its text. */
    user-select: none;
    transition: box-shadow 0.15s;
  }

  .panel input {
    user-select: text;
  }

  .panel.dropping {
    box-shadow:
      0 0 0 3px var(--accent),
      var(--lift);
  }

  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(-14px);
    }
  }

  header {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: grab;
    touch-action: none;
  }

  header:active {
    cursor: grabbing;
  }

  h3 {
    flex: 1;
    margin: 0;
    font: 700 16px var(--display);
  }

  header .text-in {
    flex: 1;
  }

  .ib {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 28px;
    height: 28px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .ib:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    border-top: 1px solid var(--rule);
  }

  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .txt small {
    font: 700 10px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .txt b {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  li .btn {
    padding: 3px 8px;
    font-size: 12px;
  }

  .empty,
  .foot {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .foot {
    font-size: 11.5px;
  }

  @media (prefers-reduced-motion: reduce) {
    .panel {
      animation: none;
    }
  }
</style>
