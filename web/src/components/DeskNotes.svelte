<script module lang="ts">
  import { onBeforeLock } from '../lib/lock';

  // The Today tab saves a moment after typing stops; a save still waiting runs before a lock.
  let pending: (() => Promise<void>) | null = null;
  onBeforeLock(async () => {
    await pending?.();
  });
</script>

<script lang="ts">
  // Notes on the Desk (round 4: notes always to hand). Jot keeps a quick thought as a Jot note,
  // Today is the day's Daily note, Meetings lists recent meeting notes. Everything lands in
  // Notes like any other note.
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { Note } from '../lib/api';
  import { dayMonth, weekdayShort } from '../lib/format';
  import { paperMenu } from '../lib/menu.svelte';
  import { logNoteSection } from '../lib/noteActions';
  import { app, attempt, openNote } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';

  type Tab = 'jot' | 'today' | 'meetings';
  const TABS: { id: Tab; label: string }[] = [
    { id: 'jot', label: 'Jot' },
    { id: 'today', label: 'Today' },
    { id: 'meetings', label: 'Meetings' },
  ];
  const SAVE_AFTER_MS = 700;

  let tab = $state<Tab>('jot');

  // ----- Jot -----
  let jot = $state('');
  let jots = $state<Note[]>([]);

  async function loadJots() {
    const list = await attempt(() => api.listNotes('', 'jot', false));
    if (list) jots = list.notes.slice(0, 4);
  }

  async function saveJot() {
    if (!jot.trim()) return;
    const note = await attempt(() => api.createNote({ kind: 'jot', title: '', body: jot, projectId: null, tags: '' }));
    if (!note) return;
    jot = '';
    jots = [note, ...jots].slice(0, 4);
    toast('Jotted down. It’s in Notes too.');
  }

  // ----- Today (the day's Daily note) -----
  let daily = $state('');
  let dailyFor = $state('');
  let saving = $state<'' | 'waiting' | 'saved'>('');
  let timer: ReturnType<typeof setTimeout> | null = null;

  async function flush() {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
    pending = null;
    const day = dailyFor;
    const body = daily;
    const ok = await attempt(() => api.saveDailyNote(day, body));
    if (ok !== undefined && dailyFor === day) saving = 'saved';
  }

  function typed() {
    saving = 'waiting';
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => void flush(), SAVE_AFTER_MS);
    pending = flush;
  }

  async function loadDaily(day: string) {
    await flush();
    const note = await attempt(() => api.dailyNote(day));
    if (app.selected !== day) return;
    daily = note?.body ?? '';
    dailyFor = day;
    saving = '';
  }

  $effect(() => {
    const day = app.selected;
    untrack(() => void loadDaily(day));
  });

  onDestroy(() => {
    void flush();
  });

  // ----- Meetings -----
  let meetings = $state<Note[]>([]);

  async function loadMeetings() {
    const list = await attempt(() => api.listNotes('', 'meeting', false));
    if (list) meetings = list.notes.slice(0, 5);
  }

  async function newMeeting() {
    const title = `Meeting ${weekdayShort(app.today)} ${dayMonth(app.today)}`;
    const note = await attempt(() => api.createNote({ kind: 'meeting', title, body: '', projectId: null, tags: '' }));
    if (note) openNote(note.id);
  }

  $effect(() => {
    if (tab === 'jot') untrack(() => void loadJots());
    if (tab === 'meetings') untrack(() => void loadMeetings());
  });
</script>

<article class="paper card">
  <h3>
    <span class="title"><Icon name="note" size={16} /> Notes</span>
    <span class="tabs" role="tablist" aria-label="Notes on the Desk">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" aria-selected={tab === t.id} onclick={() => (tab = t.id)}>{t.label}</button>
      {/each}
    </span>
  </h3>

  {#if tab === 'jot'}
    <textarea
      class="text-in pad"
      bind:value={jot}
      rows="3"
      placeholder="Jot something down. Ctrl+Enter keeps it."
      aria-label="Jot"
      onkeydown={(e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          void saveJot();
        }
      }}
    ></textarea>
    <div class="row">
      <span class="grow"></span>
      <button type="button" class="btn" disabled={!jot.trim()} onclick={saveJot}>Keep it</button>
    </div>
    {#each jots as n (n.id)}
      <button
        type="button"
        class="item"
        title="Click to open · right-click to log it as an entry"
        onclick={() => openNote(n.id)}
        use:paperMenu={() => ({
          sections: [
            { head: n.title, items: [{ label: 'Open in Notes', icon: 'open', side: true, act: () => openNote(n.id) }] },
            logNoteSection(n, () => void loadJots()),
          ],
        })}
      >
        <span class="t">{n.title}</span>
      </button>
    {/each}
  {:else if tab === 'today'}
    <textarea
      class="text-in pad"
      bind:value={daily}
      oninput={typed}
      rows="7"
      placeholder="Today's note. What happened, what's next. It saves as you type."
      aria-label="Today's note"
    ></textarea>
    <p class="status" aria-live="polite">
      {saving === 'waiting' ? 'Saving…' : saving === 'saved' ? 'Saved in Notes' : ''}
    </p>
  {:else}
    {#each meetings as n (n.id)}
      <button type="button" class="item" onclick={() => openNote(n.id)}>
        <span class="t">{n.title}</span>
      </button>
    {:else}
      <p class="empty-note">No meeting notes yet.</p>
    {/each}
    <div class="row">
      <button type="button" class="btn" onclick={newMeeting}><Icon name="plus" size={14} /> New meeting note</button>
    </div>
  {/if}
</article>

<style>
  .card {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 3px solid var(--ok);
  }

  h3 {
    margin: 0;
    font: 600 14px var(--ui);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .title {
    display: inline-flex;
    gap: 7px;
    align-items: center;
  }

  .tabs {
    display: inline-flex;
    background: color-mix(in srgb, var(--ink) 6%, transparent);
    border-radius: 7px;
    padding: 2px;
  }

  .tabs button {
    border: 0;
    background: none;
    font: 600 12px var(--ui);
    color: var(--ink-2);
    padding: 3px 9px;
    border-radius: 5px;
    cursor: pointer;
  }

  .tabs button[aria-selected='true'] {
    background: var(--paper);
    color: var(--ink);
    box-shadow: var(--shadow);
  }

  .pad {
    resize: vertical;
    min-height: 64px;
    font: 13.5px/1.55 var(--ui);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .grow {
    flex: 1;
  }

  .item {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    border-top: 1px solid var(--rule);
    background: none;
    padding: 6px 2px;
    font: 13px var(--ui);
    color: var(--ink);
    cursor: pointer;
  }

  .item:hover .t {
    color: var(--accent);
  }

  .t {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    margin: -4px 0 0;
    min-height: 1em;
    font-size: 12px;
    color: var(--mut);
  }
</style>
