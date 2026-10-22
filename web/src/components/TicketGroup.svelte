<script lang="ts">
  // An entry's tickets, opened from the strip on its Desk card (round 14): a box that floats
  // over the Desk with the entry itself first, then a card per ticket. Each ticket has the
  // round tick (done / still open) and its own notepad, like the Tickets tray; their time is
  // the entry's. More tickets can be attached at the bottom. Esc or ✕ folds it away.
  import { api } from '../lib/api';
  import type { Entry, Ticket } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { attempt, projectById, projectColor, putEntry } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let { entry, onclose }: { entry: Entry; onclose: (changed: boolean) => void } = $props();

  const SAVE_AFTER_MS = 700;
  // The entry as this box last saw it: the card's copy until a change here returns it fresh.
  let fresh = $state<Entry | null>(null);
  const shown = $derived(fresh ?? entry);
  let changed = false;
  let open = $state<string | null>(null);
  let draft = $state('');
  let adding = $state('');
  let timer: ReturnType<typeof setTimeout> | null = null;

  function took(updated: Entry | undefined) {
    if (!updated) return;
    fresh = updated;
    changed = true;
    putEntry(updated);
  }

  async function tick(t: Ticket) {
    took(await attempt(() => api.ticketSetDone(t.id, !t.done)));
  }

  function openNote(t: Ticket) {
    void flush();
    open = open === t.id ? null : t.id;
    draft = t.note ?? '';
  }

  function typed() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void flush(), SAVE_AFTER_MS);
  }

  async function flush() {
    if (timer) clearTimeout(timer);
    timer = null;
    const t = shown.tickets.find((x) => x.id === open);
    if (!t || draft.trim() === (t.note ?? '').trim()) return;
    took(await attempt(() => api.ticketSetNote(t.id, draft)));
  }

  async function add() {
    if (!adding.trim()) return;
    const text = adding;
    adding = '';
    took(await attempt(() => api.addEntryTickets(shown.id, text)));
  }

  async function close() {
    await flush();
    onclose(changed);
  }

  const open_count = $derived(shown.tickets.filter((t) => !t.done).length);
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      void close();
    }
  }}
/>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="group paper"
  aria-label="Tickets on {shown.description || 'this entry'}"
  onpointerdown={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
  onwheel={(e) => e.stopPropagation()}
>
  <header>
    <span class="head">{shown.tickets.length} ticket{shown.tickets.length === 1 ? '' : 's'}{open_count ? ` · ${open_count} still open` : ''}</span>
    <button type="button" class="x" aria-label="Close" onclick={() => void close()}><Icon name="x" size={14} /></button>
  </header>
  <div class="grid">
    <div class="sq parent" style:--c={projectColor(shown.projectId)}>
      <span class="kind">Entry · {shown.hours} h</span>
      <span class="title">{shown.description || 'Untitled'}</span>
      <span class="sub">{projectById(shown.projectId)?.name ?? 'No project'}</span>
    </div>
    {#each shown.tickets as t (t.id)}
      <div class="sq" class:still={!t.done} class:opened={open === t.id} style:--c={projectColor(shown.projectId)}>
        <span class="top">
          <button
            type="button"
            class="tick"
            class:done={t.done}
            aria-pressed={t.done}
            title={t.done ? 'Done. Click if it is still open.' : 'Still open. Click when it is done.'}
            aria-label={t.done ? `${t.number} is done; mark it still open` : `${t.number} is still open; mark it done`}
            onclick={() => void tick(t)}
          >
            {#if t.done}<Icon name="check" size={11} />{/if}
          </button>
          <span class="num mono">{t.number}</span>
          {#if !t.done}<span class="tag">Still open</span>{/if}
        </span>
        {#if open === t.id}
          <textarea
            class="pad"
            bind:value={draft}
            use:focusNow
            rows="4"
            placeholder={t.done ? 'A few words…' : 'Still open: what is left?'}
            aria-label="Notepad for {t.number}"
            oninput={typed}
            onblur={() => void flush()}
          ></textarea>
        {:else}
          <button type="button" class="note" class:muted={!t.note} onclick={() => openNote(t)} title="Open its notepad">
            {t.note?.split('\n')[0] ?? 'Click for its notepad'}
          </button>
        {/if}
      </div>
    {/each}
  </div>
  <input
    class="text-in add"
    bind:value={adding}
    placeholder="+ Attach tickets (paste one or many)"
    aria-label="Attach tickets to {shown.description || 'this entry'}"
    autocomplete="off"
    spellcheck="false"
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        void add();
      }
    }}
  />
</section>

<style>
  .group {
    position: absolute;
    width: 480px;
    max-height: 420px;
    overflow: auto;
    padding: 10px 12px 12px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 8px;
    cursor: default;
    user-select: none;
    animation: pop calc(0.22s * var(--m)) var(--spring);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .head {
    font: 700 11px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .x {
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .x:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .sq {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px 9px;
    border-radius: 5px;
    border-top: 3px solid var(--c);
    background: var(--paper);
    box-shadow: var(--shadow);
    min-height: 64px;
  }

  .sq.parent {
    background: color-mix(in srgb, var(--c) 8%, var(--paper));
  }

  .sq.still {
    border-top-color: var(--warn);
  }

  .sq.opened {
    grid-column: 1 / -1;
  }

  .kind,
  .tag {
    font: 700 10px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .tag {
    color: var(--warn);
    margin-left: auto;
  }

  .title {
    font-weight: 600;
    font-size: 13.5px;
  }

  .sub {
    font-size: 12px;
    color: var(--mut);
  }

  .top {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .num {
    font-weight: 700;
    font-size: 12.5px;
  }

  .tick {
    width: 18px;
    height: 18px;
    padding: 0;
    flex: none;
    border-radius: 50%;
    border: 2px dashed var(--warn);
    background: none;
    color: #fff;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .tick.done {
    border: 2px solid var(--ok);
    background: var(--ok);
  }

  .tick:hover {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 20%, transparent);
  }

  .note {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    font: 12.5px var(--ui);
    color: var(--ink);
    cursor: text;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note.muted {
    color: var(--mut);
    font-style: italic;
  }

  .pad {
    width: 100%;
    resize: vertical;
    border: 1px solid var(--rule);
    border-radius: 6px;
    padding: 6px 8px;
    font: 13px/1.5 var(--ui);
    background: var(--paper);
    color: var(--ink);
    user-select: text;
  }

  .add {
    user-select: text;
  }

  @media (prefers-reduced-motion: reduce) {
    .group {
      animation: none;
    }
  }
</style>
