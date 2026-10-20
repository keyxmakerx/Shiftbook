<script module lang="ts">
  // The last "open this note" request handled, so coming back to the view doesn't redo it.
  let handledOpen = 0;
</script>

<script lang="ts">
  // Notes (r7-notes card): one list with a search box, kind filters and Include archived,
  // and the open note beside it. Notes save as you type. Archive moves a note out of the
  // way; search still finds it. Auto-archive (Settings → Notes) warns on the note first.
  import { onDestroy } from 'svelte';
  import { api } from '../lib/api';
  import type { Note, NoteInput, NoteKind } from '../lib/api';
  import { archiveWarning, changedOn, highlight, KIND_LABEL, KINDS, snippet } from '../lib/notes';
  import { openMenu } from '../lib/menu.svelte';
  import { logNoteSection } from '../lib/noteActions';
  import { app, attempt, projectColor, projectName } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from '../components/Icon.svelte';

  const SAVE_AFTER_MS = 700;

  let query = $state('');
  let kind = $state<NoteKind | null>(null);
  let includeArchived = $state(false);
  let notes = $state<Note[]>([]);
  let hidden = $state(0);
  let loaded = $state(false);

  /** The open note as last saved, or null while a new one hasn't been saved yet. */
  let current = $state<Note | null>(null);
  /** True while the editor shows something (a saved note or a new one). */
  let editing = $state(false);
  let draft = $state<NoteInput>(blank('jot'));
  let saveState = $state<'' | 'waiting' | 'saving' | 'saved'>('');
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let titleEl: HTMLInputElement | undefined = $state();
  let bodyEl: HTMLTextAreaElement | undefined = $state();

  const projects = $derived(app.projects.filter((p) => !p.archived || p.id === draft.projectId));

  function blank(k: NoteKind): NoteInput {
    return { kind: k, title: '', body: '', projectId: null, tags: '' };
  }

  function toInput(n: Note): NoteInput {
    return { kind: n.kind, title: n.title, body: n.body, projectId: n.projectId, tags: n.tags.join(', ') };
  }

  async function load() {
    const q = query;
    const list = await attempt(() => api.listNotes(q, kind, includeArchived));
    if (!list || q !== query) return;
    notes = list.notes;
    hidden = list.hiddenArchived;
    loaded = true;
  }

  // Reload when the search or filters change (typing waits a moment).
  $effect(() => {
    void kind;
    void includeArchived;
    const t = setTimeout(load, query.trim() ? 120 : 0);
    return () => clearTimeout(t);
  });

  // Open a note asked for from Find or "New note".
  $effect(() => {
    const ask = app.noteOpen;
    if (ask.serial === handledOpen) return;
    handledOpen = ask.serial;
    if (ask.id) void openById(ask.id);
    else void newNote();
  });

  async function openById(id: string) {
    const n = await attempt(() => api.getNote(id));
    if (n) await select(n);
  }

  async function select(n: Note) {
    await flush();
    current = n;
    draft = toInput(n);
    editing = true;
    saveState = '';
  }

  async function newNote() {
    await flush();
    current = null;
    draft = blank(kind ?? 'jot');
    editing = true;
    saveState = '';
    queueMicrotask(() => titleEl?.focus());
  }

  function edited() {
    saveState = 'waiting';
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void save(), SAVE_AFTER_MS);
  }

  /** Save now if anything is waiting. */
  async function flush() {
    if (saveTimer === null) return;
    clearTimeout(saveTimer);
    await save();
  }

  async function save() {
    saveTimer = null;
    const input = $state.snapshot(draft);
    if (!current && !input.title.trim() && !input.body.trim()) {
      saveState = '';
      return;
    }
    saveState = 'saving';
    const was = current;
    const saved = await attempt(() => (was ? api.updateNote(was.id, input) : api.createNote(input)));
    if (!saved) {
      saveState = 'waiting';
      return;
    }
    // Only take the answer if the same note is still open.
    if (current?.id === was?.id) {
      current = saved;
      saveState = saveTimer === null ? 'saved' : 'waiting';
    }
    await load();
  }

  async function setArchived(n: Note, archived: boolean) {
    await flush();
    if (!(await attempt(() => api.setNoteArchived(n.id, archived).then(() => true)))) return;
    await refreshOpen(n.id);
    await load();
    toast(archived ? 'Archived. Search still finds it.' : 'Back with your notes.', 'Undo', () => {
      void attempt(() => api.setNoteArchived(n.id, !archived)).then(() => refreshOpen(n.id)).then(load);
    });
  }

  async function keep(n: Note) {
    if (!(await attempt(() => api.setNoteKeep(n.id, true).then(() => true)))) return;
    await refreshOpen(n.id);
    await load();
    toast("Kept. This note won't move to the archive on its own.", 'Undo', () => {
      void attempt(() => api.setNoteKeep(n.id, false)).then(() => refreshOpen(n.id)).then(load);
    });
  }

  async function letMove(n: Note) {
    if (!(await attempt(() => api.setNoteKeep(n.id, false).then(() => true)))) return;
    await refreshOpen(n.id);
    toast('This note can move to the archive on its own again.');
  }

  async function remove(n: Note) {
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = null;
    if (!(await attempt(() => api.setNoteDeleted(n.id, true).then(() => true)))) return;
    if (current?.id === n.id) {
      current = null;
      editing = false;
    }
    await load();
    toast('Note deleted', 'Undo', () => {
      void attempt(() => api.setNoteDeleted(n.id, false)).then(async () => {
        await load();
        await openById(n.id);
      });
    });
  }

  async function refreshOpen(id: string) {
    if (current?.id !== id) return;
    const n = await attempt(() => api.getNote(id));
    if (n && current?.id === id) current = n;
  }

  function setKind(k: NoteKind | null) {
    kind = k;
  }

  function onListKey(e: KeyboardEvent, i: number) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const next = i + (e.key === 'ArrowDown' ? 1 : -1);
    const el = document.querySelector<HTMLElement>(`[data-note="${next}"]`);
    el?.focus();
  }

  onDestroy(() => {
    if (saveTimer !== null) {
      clearTimeout(saveTimer);
      void save();
    }
  });
</script>

<div class="notes">
  <section class="paper list" aria-label="Notes">
    <div class="top">
      <h2>Notes</h2>
      <button type="button" class="btn primary" onclick={newNote}><Icon name="plus" size={15} /> New note</button>
    </div>
    <input
      class="text-in search"
      type="search"
      placeholder="Search every note…"
      aria-label="Search notes"
      bind:value={query}
      onkeydown={(e) => {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          document.querySelector<HTMLElement>('[data-note="0"]')?.focus();
        }
      }}
    />
    <div class="filters">
      <div class="seg" role="group" aria-label="Kind">
        <button type="button" aria-pressed={kind === null} onclick={() => setKind(null)}>All</button>
        {#each KINDS as k (k)}
          <button type="button" aria-pressed={kind === k} onclick={() => setKind(k)}>{KIND_LABEL[k]}</button>
        {/each}
      </div>
      <label class="check-row arch">
        <input type="checkbox" bind:checked={includeArchived} />
        Include archived
      </label>
    </div>

    <div class="items">
      {#each notes as n, i (n.id)}
        <button
          type="button"
          class="it"
          class:archived={n.archived}
          data-note={i}
          aria-current={current?.id === n.id ? 'true' : undefined}
          style:--c={projectColor(n.projectId)}
          onclick={() => select(n)}
          onkeydown={(e) => onListKey(e, i)}
        >
          <span class="row1">
            <b>{#each highlight(n.title, query) as p, j (j)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</b>
            <span class="kind">{KIND_LABEL[n.kind]}</span>
            {#if n.archived}<span class="kind a">Archived</span>{/if}
            {#if n.archiveInDays !== null}<span class="kind soon" title={archiveWarning(n.archiveInDays)}>Moves soon</span>{/if}
          </span>
          {#if n.body.trim()}
            <span class="snip">{#each highlight(snippet(n.body, n.title, query), query) as p, j (j)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
          {/if}
          <span class="meta">
            {changedOn(n.updatedAt, app.today)}
            {#if n.projectId}· {projectName(n.projectId)}{/if}
            {#if n.tags.length}· {n.tags.join(', ')}{/if}
          </span>
        </button>
      {/each}
      {#if hidden > 0}
        <button type="button" class="btn more" onclick={() => (includeArchived = true)}>
          {hidden} more in archived notes · show
        </button>
      {/if}
      {#if loaded && notes.length === 0 && hidden === 0}
        <p class="empty-note">
          {#if query.trim()}
            Nothing matches "{query.trim()}".
          {:else if kind}
            No {KIND_LABEL[kind]} notes yet.
          {:else}
            No notes yet. Press <b>New note</b> to write one.
          {/if}
        </p>
      {/if}
    </div>
  </section>

  <section class="paper editor" aria-label="Note">
    {#if editing}
      {#if current?.archived}
        <p class="banner">
          <Icon name="archive" size={15} /> In the archive. It's out of the way, and search still finds it.
          <button type="button" class="btn" onclick={() => current && setArchived(current, false)}>Bring back</button>
        </p>
      {:else if current && current.archiveInDays !== null}
        <p class="banner warn" role="status">
          <Icon name="clock" size={15} /> {archiveWarning(current.archiveInDays)}. Editing it, or Keep, stops that.
          <button type="button" class="btn" onclick={() => current && keep(current)}>Keep</button>
        </p>
      {/if}
      <input
        bind:this={titleEl}
        class="title"
        placeholder="Title (or leave blank to use the first line)"
        aria-label="Title"
        maxlength="200"
        bind:value={draft.title}
        oninput={edited}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            bodyEl?.focus();
          }
        }}
      />
      <div class="props">
        <label class="field">
          <span class="lbl">Kind</span>
          <select class="text-in" bind:value={draft.kind} onchange={edited}>
            {#each KINDS as k (k)}<option value={k}>{KIND_LABEL[k]}</option>{/each}
          </select>
        </label>
        <label class="field">
          <span class="lbl">Project</span>
          <select class="text-in" bind:value={draft.projectId} onchange={edited}>
            <option value={null}>No project</option>
            {#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
          </select>
        </label>
        <label class="field grow">
          <span class="lbl">Tags</span>
          <input
            class="text-in"
            placeholder="network, guest wifi"
            bind:value={draft.tags}
            oninput={edited}
          />
        </label>
      </div>
      <textarea
        bind:this={bodyEl}
        class="body"
        placeholder="Write anything…"
        aria-label="Note"
        bind:value={draft.body}
        oninput={edited}
      ></textarea>
      <div class="foot">
        <span class="saved" aria-live="polite">
          {saveState === 'saving' ? 'Saving…' : saveState === 'saved' ? 'Saved' : saveState === 'waiting' ? '…' : ''}
        </span>
        {#if current}
          {#if current.keep && (current.kind === 'jot' || current.kind === 'daily' || current.kind === 'meeting')}
            <button type="button" class="btn" onclick={() => current && letMove(current)} title="Let auto-archive move this note again">
              Kept · let it move
            </button>
          {/if}
          {#if current.kind === 'jot' && !current.archived}
            <button
              type="button"
              class="btn"
              title="Turn this jot into an entry: pick how long"
              onclick={async (e) => {
                const at = e.currentTarget as HTMLElement;
                await flush();
                if (current) openMenu(at, { sections: [logNoteSection(current, () => current && void refreshOpen(current.id).then(load))] });
              }}
            >
              <Icon name="log" size={15} />Log it as an entry
            </button>
          {/if}
          <button type="button" class="btn" onclick={() => current && setArchived(current, !current.archived)}>
            <Icon name="archive" size={15} />{current.archived ? 'Bring back' : 'Archive'}
          </button>
          <button type="button" class="btn danger" onclick={() => current && remove(current)}>
            <Icon name="trash" size={15} />Delete
          </button>
        {/if}
      </div>
    {:else}
      <div class="none">
        <p>Pick a note on the left, or start a new one.</p>
        <button type="button" class="btn primary" onclick={newNote}><Icon name="plus" size={15} /> New note</button>
        <p class="tip">Tip: <span class="kbd">Ctrl Space</span> finds notes from anywhere, archived ones too.</p>
      </div>
    {/if}
  </section>
</div>

<style>
  .notes {
    display: grid;
    grid-template-columns: minmax(280px, 380px) minmax(0, 1fr);
    gap: 14px;
    height: 100%;
    max-width: 1280px;
  }

  .list,
  .editor {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 14px;
    gap: 10px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  h2 {
    margin: 0;
    font: 650 22px var(--display);
  }

  .search {
    font-size: 15px;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
  }

  .arch {
    font-size: 13px;
  }

  .items {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0 -6px;
    padding: 0 6px 6px;
  }

  .it {
    text-align: left;
    border: 0;
    background: none;
    border-radius: 8px;
    padding: 8px 10px 8px 12px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 3px;
    border-left: 3px solid var(--c);
    color: inherit;
    transition: background 0.2s;
  }

  .it:hover,
  .it:focus-visible {
    background: color-mix(in srgb, var(--ink) 5%, transparent);
    outline: 0;
  }

  .it[aria-current='true'] {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .it.archived {
    opacity: 0.72;
  }

  .row1 {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex-wrap: wrap;
  }

  .row1 b {
    font-size: 14.5px;
  }

  .kind {
    font: 600 10.5px var(--mono);
    padding: 0 6px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink-2);
  }

  .kind.a {
    background: color-mix(in srgb, var(--none) 22%, transparent);
  }

  .kind.soon {
    background: var(--warn-wash);
    color: var(--warn);
  }

  .snip {
    font-size: 13px;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    font-size: 12px;
    color: var(--mut);
  }

  mark {
    background: color-mix(in srgb, var(--warn) 30%, transparent);
    color: inherit;
    border-radius: 2px;
  }

  .more {
    margin: 6px 4px;
    align-self: flex-start;
  }

  .banner {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--ink) 6%, transparent);
    font-size: 13.5px;
  }

  .banner.warn {
    background: var(--warn-wash);
    color: var(--warn);
  }

  .banner .btn {
    margin-left: auto;
  }

  .title {
    border: 0;
    background: none;
    outline: 0;
    font: 650 24px var(--display);
    color: var(--ink);
    padding: 4px 2px;
    border-bottom: 1px solid var(--rule);
  }

  .title:focus {
    border-bottom-color: var(--accent);
  }

  .props {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .props .grow {
    flex: 1;
    min-width: 180px;
  }

  .body {
    flex: 1;
    min-height: 240px;
    resize: none;
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--paper);
    padding: 12px 14px;
    font: 15px/1.6 var(--ui);
    color: var(--ink);
    outline: 0;
  }

  .body:focus {
    box-shadow: 0 0 0 2px var(--accent);
  }

  .foot {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
  }

  .saved {
    margin-right: auto;
    font-size: 12.5px;
    color: var(--mut);
  }

  .none {
    margin: auto;
    text-align: center;
    color: var(--mut);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .none p {
    margin: 0;
  }

  .tip {
    font-size: 12.5px;
  }

  @media (max-width: 760px) {
    .notes {
      grid-template-columns: 1fr;
      height: auto;
    }

    .items {
      max-height: 40vh;
    }
  }
</style>
