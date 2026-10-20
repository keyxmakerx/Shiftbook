<script lang="ts">
  // A board's full screen (rounds 8 and 12): step into a project, a meeting series, the Daily
  // notes or a folder from the Desk. The tree on the left shows every board; the trail on top
  // walks back up. Three views of the same things:
  //   Notebook: its notes, one open to write in (saves as you type). Links written in a note
  //             show underneath with Copy. A project also shows its time and tickets.
  //   Board:    its own canvas. Cards keep their own spots here, apart from the Desk.
  //   List:     the boards inside it, then its cards, with a box to filter them. The
  //             Archive's notes have Bring back; Done tickets lists every ticket ticked done.
  // Esc or ✕ goes back to the Desk.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { BoardChild, BoardData, CanvasCard as Card, Note } from '../lib/api';
  import { boards, closeBoard, onlyLists, openBoard, type BoardTab } from '../lib/boards.svelte';
  import { fmtMinutes, relativeDay } from '../lib/format';
  import { KIND_LABEL, snippet } from '../lib/notes';
  import { app, attempt, projectById, refresh, refreshProjects } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { openWindow } from '../lib/windows.svelte';
  import CanvasCard from './CanvasCard.svelte';
  import Icon from './Icon.svelte';

  const SAVE_AFTER_MS = 700;
  const PAD = 40;

  let data = $state<BoardData | null>(null);
  let screen: HTMLElement | undefined = $state();

  // ----- loading -----
  async function load(key: string) {
    await flush();
    const d = await attempt(() => api.board(app.today, key));
    if (boards.key !== key) return;
    if (!d) return closeBoard();
    data = d;
    if (!d.cards.some((c) => c.note?.id === current?.id)) choose(notes()[0] ?? null);
  }

  const reload = () => (boards.key ? load(boards.key) : Promise.resolve());

  $effect(() => {
    void boards.serial;
    const key = boards.key;
    if (key) void load(key);
  });

  // Going to another screen (Day, Notes…) leaves the board.
  let startView = app.view;
  $effect(() => {
    if (app.view !== startView) close();
  });

  onMount(() => {
    screen?.focus();
    return () => void flush();
  });

  async function close() {
    await flush();
    closeBoard();
  }

  // ----- notebook -----
  const notes = () =>
    (data?.cards ?? [])
      .map((c) => c.note)
      .filter((n): n is Note => !!n)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));

  let current = $state<Note | null>(null);
  /** Writing a note that doesn't exist yet (it's made on the first save). */
  let fresh = $state(false);
  let title = $state('');
  let body = $state('');
  let status = $state<'' | 'saving' | 'saved'>('');
  let timer: ReturnType<typeof setTimeout> | null = null;

  function choose(n: Note | null) {
    current = n;
    fresh = false;
    title = n?.title ?? '';
    body = n?.body ?? '';
    status = '';
  }

  function typed() {
    status = '';
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void save(), SAVE_AFTER_MS);
  }

  async function flush() {
    if (!timer) return;
    clearTimeout(timer);
    await save();
  }

  /** Put a saved note back into the board's cards, so the list and links follow. */
  function put(n: Note) {
    if (!data) return;
    const card = data.cards.find((c) => c.note?.id === n.id);
    if (card) card.note = n;
    if (current?.id === n.id) current = n;
  }

  async function save() {
    timer = null;
    if (!data) return;
    if (fresh) return void (await create());
    const n = current;
    if (!n || (title === n.title && body === n.body)) return;
    status = 'saving';
    const saved = await attempt(() =>
      api.updateNote(n.id, { kind: n.kind, title, body, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (saved) put(saved);
    status = saved ? 'saved' : '';
  }

  /** The first save of a new note makes it, in a way that puts it on this board. */
  async function create() {
    const key = data?.key ?? '';
    if (!body.trim() && !title.trim()) return;
    status = 'saving';
    let made: Note | null | undefined;
    if (key === 'daily') {
      made = await attempt(() => api.saveDailyNote(app.today, body));
    } else if (key.startsWith('project:')) {
      const projectId = key.slice('project:'.length);
      made = await attempt(() => api.createNote({ kind: 'page', title, body, projectId, tags: '' }));
    } else if (key.startsWith('folder:')) {
      made = await attempt(() => api.createNote({ kind: 'jot', title, body, projectId: null, tags: '' }));
      const n = made;
      if (n) await attempt(() => api.canvasFile({ kind: 'note', id: n.id }, key));
    }
    status = made ? 'saved' : '';
    if (!made) return;
    fresh = false;
    current = made;
    title = made.title;
    await reload();
  }

  /** New note: a page for a project, today's note for Daily or a meeting, a jot in a folder. */
  async function newNote() {
    await flush();
    const key = data?.key ?? '';
    if (key.startsWith('series:')) {
      const n = await attempt(() => api.meetingNote(key.slice('series:'.length), app.today));
      if (!n) return;
      await reload();
      choose(n);
      return;
    }
    if (key === 'daily') {
      const n = await attempt(() => api.dailyNote(app.today));
      if (n) return choose(n);
    }
    choose(null);
    fresh = true;
    setTimeout(() => screen?.querySelector<HTMLTextAreaElement>('.body')?.focus(), 0);
  }

  const newLabel = $derived(
    data?.key === 'daily'
      ? "Today's note"
      : data?.key.startsWith('series:')
        ? "Today's meeting note"
        : 'New note',
  );

  async function copyLink(link: string) {
    try {
      await navigator.clipboard.writeText(link);
      toast('Link copied. Paste it into your browser.');
    } catch {
      toast("Couldn't copy the link");
    }
  }

  // ----- the board's own canvas -----
  const bounds = $derived.by(() => {
    const cards = data?.cards ?? [];
    if (!cards.length) return { x: 0, y: 0, w: 800, h: 500 };
    const xs = cards.map((c) => c.x);
    const ys = cards.map((c) => c.y);
    const x = Math.min(...xs) - PAD;
    const y = Math.min(...ys) - PAD;
    return { x, y, w: Math.max(...xs) - x + 220 + PAD * 4, h: Math.max(...ys) - y + 160 + PAD * 4 };
  });

  async function moved(c: Card, x: number, y: number) {
    if (!data) return;
    const key = data.key;
    const at = { x: x + bounds.x, y: y + bounds.y };
    const top = Math.max(0, ...data.cards.map((o) => o.z)) + 1;
    Object.assign(c, at, { z: top, placed: true });
    await attempt(() => api.boardMove(key, c.kind, c.id, at.x, at.y));
  }

  function openInNotebook(n: Note) {
    choose(n);
    boards.tab = 'notebook';
  }

  // ----- archive -----
  async function bringBack(n: Note) {
    if (!(await attempt(() => api.setNoteArchived(n.id, false).then(() => true)))) return;
    toast(`“${n.title}” is back`, 'Undo', () => {
      void attempt(() => api.setNoteArchived(n.id, true)).then(reload);
    });
    await reload();
  }

  async function bringBackProject() {
    const id = data?.key.slice('project:'.length);
    const p = projectById(id);
    if (!p) return;
    const input = { name: p.name, color: p.color, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: false };
    if (!(await attempt(() => api.updateProject(p.id, input)))) return;
    await attempt(refreshProjects);
    void refresh();
    toast(`${p.name} is back`);
    await reload();
  }

  // ----- what things say -----
  function line(c: Card): { kind: string; text: string; day: string } {
    const day = (iso: string) => (iso === app.today ? 'Today' : relativeDay(iso, app.today));
    if (c.entry) return { kind: `Entry · ${c.entry.hours} h`, text: c.entry.description || 'Untitled', day: day(c.entry.date) };
    if (c.tray)
      return {
        kind: `${c.tray.item.done ? 'Ticket' : 'Still open'} · ${fmtMinutes(c.tray.item.minutes)}`,
        text: c.tray.item.note ? `${c.tray.item.ticket} · ${c.tray.item.note.split('\n')[0]}` : c.tray.item.ticket,
        day: day(c.tray.date),
      };
    if (c.note) return { kind: `${KIND_LABEL[c.note.kind]} note`, text: c.note.title, day: day(c.note.updatedAt.slice(0, 10)) };
    return { kind: 'Inbox scrap', text: c.inbox?.text ?? '', day: '' };
  }

  // ----- list filter -----
  let filter = $state('');
  $effect(() => {
    void boards.key;
    filter = '';
  });
  const listed = $derived.by(() => {
    const cards = data?.cards ?? [];
    const words = filter.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return cards;
    return cards.filter((c) => {
      const l = line(c);
      const text = `${l.kind} ${l.text} ${c.tray?.item.note ?? ''} ${c.entry?.tickets.map((t) => t.number).join(' ') ?? ''} ${c.note?.body ?? ''}`.toLowerCase();
      return words.every((w) => text.includes(w));
    });
  });

  const work = $derived((data?.cards ?? []).filter((c) => c.entry || c.tray));
  const tabs: { id: BoardTab; label: string }[] = [
    { id: 'notebook', label: 'Notebook' },
    { id: 'board', label: 'Board' },
    { id: 'list', label: 'List' },
  ];
  const listsOnly = $derived(!data || onlyLists(data.key));
  const tint = $derived(data?.color ?? null);

  function keydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || e.defaultPrevented) return;
    e.stopPropagation();
    void close();
  }
</script>

{#snippet branch(items: BoardChild[], depth: number)}
  {#each items as b (b.key)}
    <li>
      <button
        type="button"
        class="node"
        class:here={b.key === data?.key}
        style:--c={b.color ?? 'var(--mut)'}
        style:padding-left="{10 + depth * 14}px"
        aria-current={b.key === data?.key ? 'page' : undefined}
        onclick={() => openBoard(b.key)}
      >
        <span class="dot" aria-hidden="true"></span>
        <span class="nm">{b.title}</span>
        {#if b.count}<span class="n">{b.count}</span>{/if}
      </button>
      {#if b.children.length}
        <ul>{@render branch(b.children, depth + 1)}</ul>
      {/if}
    </li>
  {/each}
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="screen"
  class:tinted={!!tint}
  style:--tint={tint ?? 'var(--accent)'}
  bind:this={screen}
  tabindex="-1"
  aria-label="{data?.title ?? 'Board'} board"
  onkeydown={keydown}
>
  <header class="top">
    <nav class="trail" aria-label="Where you are">
      <button type="button" class="crumb" onclick={() => void close()} title="Back to the Desk">
        <Icon name="desk" size={14} />Desk
      </button>
      {#each data?.trail.slice(1) ?? [] as c, i (c.key)}
        <span class="sep" aria-hidden="true">›</span>
        {#if i === (data?.trail.length ?? 0) - 2}
          <span class="crumb here" aria-current="page">{c.title}</span>
        {:else}
          <button type="button" class="crumb" onclick={() => openBoard(c.key)}>{c.title}</button>
        {/if}
      {/each}
    </nav>
    {#if !listsOnly}
      <div class="tabs" role="tablist" aria-label="View">
        {#each tabs as t (t.id)}
          <button
            type="button"
            role="tab"
            class="tab"
            class:on={boards.tab === t.id}
            aria-selected={boards.tab === t.id}
            onclick={() => (boards.tab = t.id)}>{t.label}</button
          >
        {/each}
      </div>
    {/if}
    <button type="button" class="close" onclick={() => void close()} aria-label="Close the board" title="Back to the Desk (Esc)">
      <Icon name="x" size={16} />
    </button>
  </header>

  <div class="body-wrap">
    <aside class="tree" aria-label="All boards">
      <button type="button" class="node all" class:here={data?.key === 'desk'} onclick={() => openBoard('desk')}>
        <Icon name="desk" size={13} /><span class="nm">All boards</span>
      </button>
      <ul>{@render branch(data?.tree ?? [], 0)}</ul>
    </aside>

    <main class="main">
      {#if !data}
        <p class="lead pad">Opening…</p>
      {:else}
        {#if data.archived}
          <p class="banner">
            <Icon name="archive" size={15} />This project is archived. Its history stays.
            <button type="button" class="btn slim" onclick={() => void bringBackProject()}>Bring back</button>
          </p>
        {/if}

        {#if listsOnly || boards.tab === 'list'}
          <div class="list">
            {#if data.children.length}
              <h2 class="lbl">{data.key === 'archive' ? 'Archived projects' : 'Boards'}</h2>
              <ul class="rows">
                {#each data.children as b (b.key)}
                  <li>
                    <button type="button" class="row board" style:--c={b.color ?? 'var(--mut)'} onclick={() => openBoard(b.key)}>
                      <span class="dot" aria-hidden="true"></span>
                      <span class="txt">{b.title}</span>
                      <span class="meta">{b.count} {b.count === 1 ? 'thing' : 'things'}</span>
                      <Icon name="right" size={14} />
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if data.cards.length}
              <div class="list-head">
                <h2 class="lbl">{data.key === 'archive' ? 'Archived notes' : data.key === 'done' ? 'Ticked done, newest first' : 'On this board'}</h2>
                {#if data.cards.length > 5}
                  <input
                    class="filter"
                    type="search"
                    bind:value={filter}
                    placeholder={data.key === 'done' ? 'Filter by ticket or note' : 'Filter'}
                    aria-label="Filter this list"
                  />
                {/if}
              </div>
              {#if !listed.length}<p class="empty">Nothing here matches “{filter}”.</p>{/if}
              <ul class="rows">
                {#each listed as c (`${c.kind}:${c.id}`)}
                  {@const l = line(c)}
                  <li class="row">
                    <span class="kind">{l.kind}</span>
                    <span class="txt">{l.text}</span>
                    <span class="meta">{l.day}</span>
                    {#if data.key === 'archive' && c.note}
                      {@const n = c.note}
                      <button type="button" class="btn slim" onclick={() => void bringBack(n)}>
                        <Icon name="undo" size={13} />Bring back
                      </button>
                    {:else if c.note}
                      {@const n = c.note}
                      <button type="button" class="btn slim ghost" onclick={() => openInNotebook(n)}>Open</button>
                    {:else if c.entry || c.tray}
                      <button type="button" class="btn slim ghost" onclick={() => openWindow(c.kind, c.id)}>Notepad</button>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
            {#if !data.children.length && !data.cards.length}
              <p class="empty">
                {data.key === 'archive'
                  ? 'Nothing is archived. Notes you archive, and ones left alone for a while, wait here.'
                  : data.key === 'done'
                    ? 'No tickets ticked done in the last 90 days. Older ones turn up in Find.'
                    : 'Nothing here yet.'}
              </p>
            {/if}
          </div>
        {:else if boards.tab === 'notebook'}
          <div class="notebook" class:with-side={data.key.startsWith('project:')}>
            <div class="pages">
              <button type="button" class="btn new" onclick={() => void newNote()}>
                <Icon name="plus" size={14} />{newLabel}
              </button>
              <ul class="note-list">
                {#each notes() as n (n.id)}
                  <li>
                    <button type="button" class="page" class:here={!fresh && current?.id === n.id} onclick={() => void flush().then(() => choose(n))}>
                      <span class="pt">{n.title}</span>
                      <span class="ps">{snippet(n.body, n.title, '', 60) || KIND_LABEL[n.kind]}</span>
                    </button>
                  </li>
                {:else}
                  <li class="empty tight">No notes here yet.</li>
                {/each}
              </ul>
            </div>
            <div class="editor">
              {#if current || fresh}
                <input
                  class="title-in"
                  bind:value={title}
                  oninput={typed}
                  placeholder={fresh ? 'Title (or leave blank to use the first line)' : 'Title'}
                  aria-label="Note title"
                  disabled={data.key === 'daily' || data.key.startsWith('series:') || current?.kind === 'daily'}
                />
                <textarea class="body" bind:value={body} oninput={typed} placeholder="Write here. It saves as you type." aria-label="Note"></textarea>
                <div class="foot">
                  <span class="status" aria-live="polite">{status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved' : ''}</span>
                  {#if current}
                    {@const n = current}
                    <button type="button" class="btn slim ghost" onclick={() => openWindow('note', n.id)}>
                      <Icon name="expand" size={13} />Pop out
                    </button>
                  {/if}
                </div>
                {#if current?.links.length}
                  <div class="links" aria-label="Links in this note">
                    {#each current.links as link (link)}
                      <span class="linkchip" title={link}>
                        <Icon name="link" size={12} /><span class="url">{link}</span>
                        <button type="button" class="copy" onclick={() => void copyLink(link)} aria-label="Copy {link}">
                          <Icon name="copy" size={12} />Copy
                        </button>
                      </span>
                    {/each}
                  </div>
                {/if}
              {:else}
                <p class="empty">Pick a note on the left, or start one with “{newLabel}”.</p>
              {/if}
            </div>
            {#if data.key.startsWith('project:')}
              <aside class="side" aria-label="Time and tickets">
                <div class="stat"><span class="big">{data.hours ?? '0'} h</span><span class="cap">logged in all</span></div>
                <div class="stat">
                  <span class="big" class:warn={data.openTickets > 0}>{data.openTickets}</span><span class="cap">tickets still open</span>
                </div>
                <h3 class="lbl">Lately</h3>
                <ul class="mini">
                  {#each work.slice(0, 12) as c (`${c.kind}:${c.id}`)}
                    {@const l = line(c)}
                    <li>
                      <button type="button" class="mini-row" onclick={() => openWindow(c.kind, c.id)} title="Open its notepad">
                        <span class="kind">{l.kind}</span>
                        <span class="txt">{l.text}</span>
                      </button>
                    </li>
                  {:else}
                    <li class="empty tight">No entries or tickets in the last 90 days.</li>
                  {/each}
                </ul>
              </aside>
            {/if}
          </div>
        {:else}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="plane" aria-label="{data.title} board" ondragstart={(e) => e.preventDefault()}>
            {#if !data.cards.length}
              <p class="empty pad">Nothing on this board yet.</p>
            {/if}
            <div class="sheet" style:width="{bounds.w}px" style:height="{bounds.h}px">
              {#each data.cards as c (`${c.kind}:${c.id}`)}
                <CanvasCard
                  card={c}
                  x={c.x - bounds.x}
                  y={c.y - bounds.y}
                  zoom={1}
                  onmove={(x, y) => void moved(c, x, y)}
                  onchanged={() => void reload()}
                  onopen={c.note ? () => openInNotebook(c.note!) : undefined}
                />
              {/each}
            </div>
          </div>
        {/if}
      {/if}
    </main>
  </div>
</section>

<style>
  .screen {
    position: absolute;
    inset: 0;
    z-index: 30;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    outline: none;
    animation: rise calc(0.28s * var(--m)) var(--ease);
  }

  .screen.tinted {
    background: color-mix(in srgb, var(--tint) 7%, var(--bg));
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.99);
    }
  }

  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--tint) 10%, var(--paper));
  }

  .trail {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    padding: 5px 8px;
    border-radius: 7px;
    font: 600 14px var(--ui);
    color: var(--ink-2);
    cursor: pointer;
    white-space: nowrap;
  }

  button.crumb:hover {
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }

  .crumb.here {
    color: var(--ink);
    font-weight: 700;
    font-size: 16px;
    cursor: default;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sep {
    color: var(--mut);
  }

  .tabs {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }

  .tab {
    border: 0;
    background: none;
    padding: 6px 14px;
    border-radius: 8px;
    font: 600 13px var(--ui);
    color: var(--ink-2);
    cursor: pointer;
  }

  .tab.on {
    background: var(--paper);
    color: var(--ink);
    box-shadow: var(--shadow);
  }

  .close {
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .close:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .body-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .tree {
    width: 230px;
    flex: none;
    overflow: auto;
    padding: 10px 8px;
    border-right: 1px solid var(--line);
  }

  .tree ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .node {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    background: none;
    padding: 6px 10px;
    border-radius: 7px;
    font: 500 13.5px var(--ui);
    color: var(--ink);
    text-align: left;
    cursor: pointer;
  }

  .node:hover {
    background: color-mix(in srgb, var(--ink) 6%, transparent);
  }

  .node.here {
    background: color-mix(in srgb, var(--tint) 16%, transparent);
    font-weight: 700;
  }

  .node.all {
    margin-bottom: 4px;
    font-weight: 600;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
    flex: none;
  }

  .nm,
  .txt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .n {
    font: 600 11px var(--ui);
    color: var(--mut);
  }

  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    position: relative;
  }

  .pad {
    padding: 24px;
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 20px 0;
    padding: 10px 14px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--warn) 14%, var(--paper));
  }

  .banner .btn {
    margin-left: auto;
  }

  /* ----- list ----- */
  .list {
    max-width: 860px;
    padding: 18px 24px 40px;
  }

  .lbl {
    font: 700 11px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
    margin: 16px 0 8px;
  }

  .list-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
  }

  .filter {
    margin: 0 0 8px;
    width: 240px;
    padding: 6px 10px;
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--paper);
    font: 13.5px var(--ui);
    color: var(--ink);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 10px;
    background: var(--paper);
    box-shadow: var(--shadow);
    font: 500 14px var(--ui);
    color: var(--ink);
  }

  .row.board {
    width: 100%;
    border: 0;
    border-left: 4px solid var(--c);
    text-align: left;
    cursor: pointer;
  }

  .row.board:hover {
    box-shadow: var(--lift);
  }

  .kind {
    font: 700 10px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mut);
    white-space: nowrap;
  }

  .meta {
    font-size: 12.5px;
    color: var(--mut);
    white-space: nowrap;
  }

  .empty {
    color: var(--mut);
    font-size: 14px;
  }

  .empty.tight {
    font-size: 12.5px;
    padding: 6px 4px;
  }

  /* ----- notebook ----- */
  .notebook {
    display: grid;
    grid-template-columns: 250px 1fr;
    height: 100%;
    min-height: 0;
  }

  .notebook.with-side {
    grid-template-columns: 250px 1fr 250px;
  }

  .pages {
    border-right: 1px solid var(--line);
    padding: 12px 10px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .new {
    justify-content: center;
  }

  .note-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .page {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 0;
    border-radius: 8px;
    padding: 8px 10px;
    background: none;
    text-align: left;
    cursor: pointer;
    color: var(--ink);
  }

  .page:hover {
    background: color-mix(in srgb, var(--ink) 6%, transparent);
  }

  .page.here {
    background: var(--paper);
    box-shadow: var(--shadow);
  }

  .pt {
    font: 600 13.5px var(--ui);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ps {
    font-size: 12px;
    color: var(--mut);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 22px;
    min-height: 0;
  }

  .title-in {
    font: 700 20px var(--ui);
    border: 0;
    border-bottom: 1px solid var(--line);
    background: none;
    padding: 6px 2px;
    color: var(--ink);
  }

  .body {
    flex: 1;
    min-height: 200px;
    resize: none;
    border: 0;
    border-radius: 10px;
    padding: 14px 16px;
    background: var(--paper);
    box-shadow: var(--shadow);
    font: 15px/1.6 var(--ui);
    color: var(--ink);
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status {
    flex: 1;
    font-size: 12.5px;
    color: var(--mut);
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .linkchip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    padding: 3px 4px 3px 10px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 12%, var(--paper));
    font-size: 12.5px;
    color: var(--ink);
  }

  .url {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .copy {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 0;
    border-radius: 10px;
    padding: 3px 8px;
    background: var(--paper);
    font: 600 11.5px var(--ui);
    color: var(--ink);
    cursor: pointer;
  }

  .side {
    border-left: 1px solid var(--line);
    padding: 16px 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .big {
    font: 700 22px var(--ui);
  }

  .big.warn {
    color: #e07b1a;
  }

  .cap {
    font-size: 12.5px;
    color: var(--mut);
  }

  .slim {
    min-height: 28px;
    padding: 3px 10px;
    font-size: 12.5px;
    white-space: nowrap;
  }

  .ghost {
    box-shadow: none;
  }

  .mini {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .mini-row {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 1px;
    border: 0;
    border-radius: 7px;
    padding: 6px 8px;
    background: var(--paper);
    text-align: left;
    cursor: pointer;
    font: 500 13px var(--ui);
    color: var(--ink);
  }

  .mini-row:hover {
    box-shadow: var(--shadow);
  }

  /* ----- board ----- */
  .plane {
    position: absolute;
    inset: 0;
    overflow: auto;
    background-image: radial-gradient(color-mix(in srgb, var(--ink) 14%, transparent) 1px, transparent 1px);
    background-size: 22px 22px;
  }

  .sheet {
    position: relative;
  }

  @media (max-width: 900px) {
    .tree {
      display: none;
    }

    .notebook.with-side {
      grid-template-columns: 220px 1fr;
    }

    .side {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .screen {
      animation: none;
    }
  }
</style>
