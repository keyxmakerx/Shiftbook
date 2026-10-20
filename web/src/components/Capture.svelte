<script lang="ts">
  // Capture (Ctrl+Space): one line such as "1.5h firewall cleanup CHG0012345 #network
  // yesterday". The app reads it as you type (crates/core/src/capture.rs) and shows chips.
  // A line with hours logs on Enter. Anything else is a search (round 7): matching
  // entries, Inbox scraps and commands appear below, ranked by crates/core/src/search.rs.
  // Ctrl+Enter, or Enter with nothing found, saves the line to the Inbox.
  import { api } from '../lib/api';
  import type { Capture, Entry, InboxItem, Note, TrayItem } from '../lib/api';
  import { KIND_LABEL } from '../lib/notes';
  import { dayOfMonth, fmtMinutes, relativeDay, weekdayShort } from '../lib/format';
  import { allowCapture, protectNow } from '../lib/protection.svelte';
  import {
    app,
    attempt,
    flashEntry,
    focusLog,
    openNote,
    projectById,
    projectColor,
    refresh,
    refreshInbox,
    selectDay,
    setView,
  } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { trapTab } from '../lib/trap';
  import { openWindow } from '../lib/windows.svelte';

  let { onLock }: { onLock: () => void } = $props();

  type Command = { label: string; keys?: string; run: () => void | Promise<void> };
  type Hit =
    | { kind: 'entry'; entry: Entry }
    | { kind: 'scrap'; item: InboxItem }
    | { kind: 'note'; note: Note }
    | { kind: 'ticket'; ticket: TrayItem }
    | { kind: 'command'; command: Command };

  const COMMANDS: Command[] = [
    { label: 'Go to Desk', keys: '1', run: () => setView('desk') },
    { label: 'Go to Day', keys: '2', run: () => setView('day') },
    { label: 'Go to Timesheet', keys: '3', run: () => setView('timesheet') },
    { label: 'Go to Notes', keys: '4', run: () => setView('notes') },
    { label: 'Go to Settings', keys: '5', run: () => setView('settings') },
    { label: 'New note', run: () => openNote(null) },
    { label: 'Lock now', keys: 'Ctrl L', run: () => onLock() },
    { label: 'Book today as holiday (8 h)', run: () => holiday() },
    {
      label: 'Allow screenshots for 10 minutes',
      run: () => attempt(allowCapture).then(() => toast('Screenshots and copying are allowed for 10 minutes.')),
    },
    { label: 'Protect now (block screenshots)', run: () => attempt(protectNow).then(() => toast('Protected again.')) },
  ];

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let hits = $state<Hit[]>([]);
  let sel = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let box: HTMLElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | null = null;

  const logging = $derived(!!seen && seen.minutes !== null);

  $effect(() => {
    if (app.captureOpen) {
      text = '';
      seen = null;
      hits = [];
      sel = 0;
      queueMicrotask(() => input?.focus());
    }
  });

  function read() {
    if (timer !== null) clearTimeout(timer);
    const now = text;
    if (!now.trim()) {
      seen = null;
      hits = [];
      return;
    }
    timer = setTimeout(async () => {
      const [c, found] = await Promise.all([
        api.parseCapture(now, app.today).catch(() => null),
        api
          .search(
            now,
            app.today,
            COMMANDS.map((c) => c.label),
          )
          .catch(() => null),
      ]);
      if (text !== now) return;
      seen = c;
      hits = found
        ? [
            ...found.entries.map((entry): Hit => ({ kind: 'entry', entry })),
            ...found.tickets.map((ticket): Hit => ({ kind: 'ticket', ticket })),
            ...found.notes.map((note): Hit => ({ kind: 'note', note })),
            ...found.inbox.map((item): Hit => ({ kind: 'scrap', item })),
            ...found.commands.flatMap((i): Hit[] => {
              const command = COMMANDS[i];
              return command ? [{ kind: 'command', command }] : [];
            }),
          ]
        : [];
      sel = 0;
    }, 90);
  }

  function close() {
    app.captureOpen = false;
  }

  async function holiday() {
    const date = app.today;
    const entry = await attempt(() =>
      api.createEntry(
        { date, minutes: 480, description: 'Holiday', projectId: null, category: null, notes: null, timesheetCodeId: null },
        '',
      ),
    );
    if (!entry) return;
    setView('day');
    await selectDay(date);
    await refresh();
    flashEntry(entry.id);
    toast('Today booked as holiday', 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    });
  }

  async function toInbox(line: string) {
    const item = await attempt(() => api.addInbox(line));
    if (!item) return;
    close();
    await attempt(refreshInbox);
    toast('Saved to the Inbox', 'Undo', () => {
      void attempt(() => api.setInboxDeleted(item.id, true)).then(() => attempt(refreshInbox));
    });
  }

  async function open(hit: Hit) {
    close();
    if (hit.kind === 'command') {
      await hit.command.run();
    } else if (hit.kind === 'note') {
      openNote(hit.note.id);
    } else if (hit.kind === 'ticket') {
      openWindow('tray', hit.ticket.id);
    } else if (hit.kind === 'entry') {
      setView('day');
      await selectDay(hit.entry.date);
      flashEntry(hit.entry.id);
      app.drawer = hit.entry.id;
    } else {
      // A scrap moves into Log work so a time can be picked, like "Log it" on the Inbox card.
      const id = hit.item.id;
      if (await attempt(() => api.setInboxDeleted(id, true).then(() => true))) await attempt(refreshInbox);
      focusLog(hit.item.text);
    }
  }

  async function submit(toInboxAnyway: boolean) {
    const line = text.trim();
    if (!line) return;
    if (!toInboxAnyway && !logging && hits[sel]) {
      await open(hits[sel]);
      return;
    }
    const parsed = seen ?? (await api.parseCapture(line, app.today).catch(() => null));
    if (toInboxAnyway || !parsed || parsed.minutes === null) {
      await toInbox(line);
      return;
    }
    const entry = await attempt(() => api.captureLog(line, app.today));
    if (!entry) return;
    close();
    if (app.view !== 'desk' && app.view !== 'day') setView('desk');
    await selectDay(entry.date);
    await refresh();
    flashEntry(entry.id);
    const when = entry.date === app.today ? 'today' : `on ${weekdayShort(entry.date)} ${dayOfMonth(entry.date)}`;
    toast(`Logged ${fmtMinutes(parsed.minutes)} ${when}`, 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    });
  }

  function move(step: number) {
    if (hits.length === 0) return;
    sel = (sel + step + hits.length) % hits.length;
    box?.querySelector(`[data-hit="${sel}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  function dayText(c: Capture): string {
    return c.date === app.today ? 'today' : `${weekdayShort(c.date)} ${dayOfMonth(c.date)}`;
  }

  function group(hit: Hit): string {
    return hit.kind === 'entry'
      ? 'Entries'
      : hit.kind === 'ticket'
        ? 'Tickets'
        : hit.kind === 'note'
          ? 'Notes'
          : hit.kind === 'scrap'
            ? 'Inbox'
            : 'Commands';
  }
</script>

<div class="scrim" class:on={app.captureOpen} onclick={close} aria-hidden="true"></div>
<svelte:window onkeydown={(e) => app.captureOpen && trapTab(e, box)} />
<div
  bind:this={box}
  class="paper capture"
  class:on={app.captureOpen}
  role="dialog"
  aria-modal="true"
  aria-label="Log, note or find"
  aria-hidden={!app.captureOpen}
>
  <input
    bind:this={input}
    bind:value={text}
    oninput={read}
    onkeydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        close();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        void submit(e.ctrlKey || e.metaKey);
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        move(e.key === 'ArrowDown' ? 1 : -1);
      }
    }}
    tabindex={app.captureOpen ? 0 : -1}
    autocomplete="off"
    spellcheck="false"
    aria-label="Log, note or find"
    aria-controls="capture-hits"
    aria-activedescendant={!logging && hits[sel] ? `hit-${sel}` : undefined}
    placeholder="Log “1.5h printer INC0045678”, or find a ticket, a word or a command"
  />
  <div class="toks" class:quiet={!logging && hits.length > 0} aria-live="polite">
    {#if seen && text.trim() && logging}
      <span class="tok" style:--c="var(--ok)"><em>time</em>{fmtMinutes(seen.minutes ?? 0)}</span>
      {#if seen.projectId}
        <span class="tok" style:--c={projectColor(seen.projectId)}><em>project</em>{projectById(seen.projectId)?.name}</span>
      {:else if seen.projectQuery}
        <span class="tok" style:--c="var(--warn)"><em>project?</em>no “{seen.projectQuery}”</span>
      {/if}
      {#each seen.tickets as t (t)}
        <span class="tok" style:--c="var(--accent)"><em>ticket</em>{t}</span>
      {/each}
      <span class="tok"><em>day</em>{dayText(seen)}</span>
      {#if seen.description}
        <span class="tok" style:--c="var(--ink)"><em>what</em>{seen.description}</span>
      {/if}
    {:else if text.trim() && hits.length === 0}
      <span class="none">Nothing found. <span class="kbd">Enter</span> saves it to the Inbox; add a time like 30m to log it.</span>
    {/if}
  </div>
  {#if !logging && hits.length > 0}
    <ul class="hits" id="capture-hits" role="listbox" aria-label="Found">
      {#each hits as hit, i (i)}
        {#if i === 0 || group(hits[i - 1]!) !== group(hit)}
          <li class="lbl head" role="presentation">{group(hit)}</li>
        {/if}
        <li
          id="hit-{i}"
          data-hit={i}
          role="option"
          aria-selected={i === sel}
          class="hit"
          class:on={i === sel}
          onpointermove={() => (sel = i)}
          onpointerdown={(e) => {
            e.preventDefault();
            void open(hit);
          }}
        >
          {#if hit.kind === 'entry'}
            <i class="dot" style:--c={projectColor(hit.entry.projectId)}></i>
            <span class="what"><b>{hit.entry.description || 'Untitled'}</b>
              <small>{relativeDay(hit.entry.date, app.today)}{hit.entry.tickets.length ? ' · ' + hit.entry.tickets.map((t) => t.number).join(', ') : ''}</small></span>
            <span class="mono amt">{hit.entry.hours} h</span>
          {:else if hit.kind === 'ticket'}
            {@const t = hit.ticket}
            <i class="dot" style:--c={t.projectId ? projectColor(t.projectId) : 'var(--accent)'}></i>
            <span class="what"><b class="mono">{t.ticket}</b>
              <small>{relativeDay(t.date, app.today)} · {t.done ? 'done' : 'still open'}{t.note ? ' · ' + t.note.split('\n')[0] : ''}</small></span>
            <span class="mono amt">{fmtMinutes(t.minutes)}</span>
          {:else if hit.kind === 'note'}
            <i class="dot" style:--c={projectColor(hit.note.projectId)}></i>
            <span class="what"><b>{hit.note.title}</b>
              <small>{KIND_LABEL[hit.note.kind]}{hit.note.archived ? ' · archived' : ''}{hit.note.tags.length ? ' · ' + hit.note.tags.join(', ') : ''}</small></span>
          {:else if hit.kind === 'scrap'}
            <i class="dot" style:--c="var(--warn)"></i>
            <span class="what"><b>{hit.item.text}</b><small>Inbox · Enter puts it in Log work</small></span>
          {:else}
            <i class="dot cmd" aria-hidden="true">›</i>
            <span class="what"><b>{hit.command.label}</b></span>
            {#if hit.command.keys}<span class="kbd">{hit.command.keys}</span>{/if}
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <div class="foot">
    {#if logging}
      <span><span class="kbd">Enter</span> log it</span>
    {:else if hits.length > 0}
      <span><span class="kbd">↑ ↓</span> choose</span>
      <span><span class="kbd">Enter</span> open</span>
    {:else}
      <span><span class="kbd">Enter</span> log it, or save to the Inbox</span>
    {/if}
    <span><span class="kbd">Ctrl Enter</span> to Inbox</span>
    <span><span class="kbd">Esc</span> close</span>
    <span class="tip">Add a time like 30m and it logs instead</span>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: color-mix(in srgb, var(--desk) 40%, transparent);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.2s;
  }

  .scrim.on {
    opacity: 1;
    pointer-events: auto;
  }

  .capture {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 92px;
    width: min(660px, calc(100% - 32px));
    transform: translate(-50%, -14px) scale(0.96);
    opacity: 0;
    pointer-events: none;
    visibility: hidden;
    transition:
      transform calc(0.35s * var(--m)) var(--spring),
      opacity 0.2s,
      visibility 0s 0.2s;
    overflow: hidden;
    padding: 0;
  }

  .capture.on {
    transform: translateX(-50%);
    opacity: 1;
    pointer-events: auto;
    visibility: visible;
    transition:
      transform calc(0.35s * var(--m)) var(--spring),
      opacity 0.2s;
  }

  input {
    width: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    padding: 16px 18px;
    font: 500 18px var(--ui);
  }

  .toks {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    padding: 0 18px 12px;
    min-height: 32px;
  }

  /* Results take the space when there are some. */
  .toks.quiet {
    min-height: 0;
    padding-bottom: 0;
  }

  .tok {
    font: 600 12px var(--ui);
    padding: 3px 9px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--c, var(--ink)) 13%, transparent);
    color: color-mix(in srgb, var(--c, var(--ink)) 85%, var(--ink));
    animation: pop 0.3s var(--bounce);
  }

  .tok em {
    font-style: normal;
    opacity: 0.65;
    font-weight: 500;
    margin-right: 4px;
  }

  @keyframes pop {
    from {
      transform: scale(0.6);
      opacity: 0;
    }
  }

  .foot {
    display: flex;
    gap: 16px;
    padding: 9px 18px;
    border-top: 1px solid var(--rule);
    font-size: 12px;
    color: var(--mut);
    flex-wrap: wrap;
  }

  .tip {
    margin-left: auto;
  }

  .none {
    font-size: 12.5px;
    color: var(--mut);
  }

  .hits {
    list-style: none;
    margin: 0;
    padding: 0 8px 8px;
    max-height: min(52vh, 420px);
    overflow-y: auto;
  }

  .head {
    padding: 8px 10px 3px;
  }

  .hit {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 8px;
    cursor: pointer;
  }

  .hit.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .what b {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .what small {
    color: var(--mut);
    font-size: 12px;
  }

  .amt {
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .cmd {
    background: none;
    color: var(--mut);
    font-style: normal;
    font-weight: 700;
    width: 10px;
    text-align: center;
    line-height: 10px;
  }
</style>
