<script lang="ts">
  // Capture (Ctrl+Space): one line such as "1.5h firewall cleanup CHG0012345 #network
  // yesterday". The app reads it as you type (crates/core/src/capture.rs) and shows chips.
  // A line with hours logs on Enter. Anything else is a search (round 7): matching
  // entries, Inbox scraps and commands appear below, ranked by crates/core/src/search.rs.
  // Ctrl+Enter, or Enter with nothing found, saves the line to the Inbox. Round 15: "/" opens
  // it too, every action the app has is a command here (grouped Go to / Do / Settings), a
  // day such as "last friday" or "12 oct" offers that day, and "help" lists everything.
  import { api } from '../lib/api';
  import type { Capture, Entry, InboxItem, MeetingSeries, Note, TrayItem } from '../lib/api';
  import { openBoard } from '../lib/boards.svelte';
  import { toIsoDate } from '../lib/dates';
  import { KIND_LABEL } from '../lib/notes';
  import { dayMonth, dayOfMonth, fmtMinutes, longDay, relativeDay, weekdayShort } from '../lib/format';
  import type { SettingsSection } from '../lib/keys';
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
    ribbonDay,
    ribbonWeekOf,
    selectDay,
    setView,
  } from '../lib/store.svelte';
  import { copyStandup } from '../lib/standup';
  import { startTimer, stopTimer, timer as running } from '../lib/timer.svelte';
  import { changes, toast, undoBack } from '../lib/toast.svelte';
  import { agoText, type Change } from '../lib/undoHistory';
  import { trapTab } from '../lib/trap';
  import { openWindow } from '../lib/windows.svelte';
  import { phone } from '../lib/phone.svelte';
  import { launch } from '../lib/launch.svelte';

  let { onLock }: { onLock: () => void } = $props();

  type Group = 'Go to' | 'Do' | 'Settings';
  type Command = {
    label: string;
    group: Group;
    keys?: string;
    /** Extra words Find matches but doesn't show. */
    words?: string;
    run: () => void | Promise<void>;
  };
  type Hit =
    | { kind: 'entry'; entry: Entry }
    | { kind: 'scrap'; item: InboxItem }
    | { kind: 'note'; note: Note }
    | { kind: 'ticket'; ticket: TrayItem }
    | { kind: 'day'; date: string; sheet: boolean }
    | { kind: 'change'; change: Change; after: number }
    | { kind: 'command'; command: Command };

  const GROUPS: Group[] = ['Go to', 'Do', 'Settings'];
  /** Most commands shown for a search; "help" shows them all. */
  const MAX_COMMANDS = 8;

  const SETTINGS: { id: SettingsSection; label: string; words: string }[] = [
    { id: 'look', label: 'Look', words: 'theme colour color dark light text size motion' },
    { id: 'day', label: 'Your day', words: 'target hours wrap-up timer stand-up time buttons' },
    { id: 'projects', label: 'Projects', words: 'colours archive' },
    { id: 'timesheet', label: 'Timesheet', words: 'codes grouping' },
    { id: 'notes', label: 'Notes', words: 'kinds templates' },
    { id: 'fields', label: 'Fields', words: 'custom' },
    { id: 'responses', label: 'Responses', words: 'canned replies' },
    { id: 'meetings', label: 'Meetings', words: 'series bell' },
    { id: 'backups', label: 'Backups', words: 'restore import export' },
    { id: 'security', label: 'Security', words: 'password recovery key lock pin' },
  ];

  let series = $state<MeetingSeries[]>([]);

  function onDesk(ask: 'fit' | 'tidy' | 'timeline') {
    setView('desk');
    app.deskAsk = ask;
  }

  function settings(id: SettingsSection) {
    app.settingsSection = id;
    setView('settings');
  }

  async function backupNow() {
    const name = await attempt(() => api.backupNow(toIsoDate(new Date())));
    if (name) toast(`Backup saved as “${name}”.`);
  }

  function wrapUp() {
    app.wrapupClosed = '';
    app.wrapupNow = app.today;
    if (app.view !== 'desk' && app.view !== 'day') setView('desk');
    void selectDay(app.today);
  }

  async function timerCommand() {
    if (running.target) {
      await stopTimer();
      return;
    }
    const today = (await attempt(() => api.listEntries(app.today, app.today))) ?? [];
    const last = today.filter((e) => !e.isTray).pop();
    if (last) await startTimer(last);
    else toast('Log something today first, then the timer can run on it.');
  }

  const commands = $derived.by((): Command[] => [
    { label: 'Go to Desk', group: 'Go to', keys: '1', run: () => setView('desk') },
    { label: 'Go to Day', group: 'Go to', keys: '2', run: () => setView('day') },
    { label: 'Go to Timesheet', group: 'Go to', keys: '3', run: () => setView('timesheet') },
    { label: 'Go to Notes', group: 'Go to', keys: '4', run: () => setView('notes') },
    { label: 'Go to Settings', group: 'Go to', keys: '5', run: () => setView('settings') },
    { label: 'Boards', group: 'Go to', words: 'full screen projects meetings', run: () => openBoard('desk') },
    { label: 'Daily notes', group: 'Go to', words: 'board', run: () => openBoard('daily') },
    {
      label: 'Done tickets',
      group: 'Go to',
      words: 'board closed items',
      run: () => {
        app.itemsTab = 'done';
        setView('items');
      },
    },
    { label: 'Archive', group: 'Go to', words: 'board archived', run: () => openBoard('archive') },
    ...app.projects
      .filter((p) => !p.archived)
      .map((p): Command => ({ label: `Project board: ${p.name}`, group: 'Go to', run: () => openBoard(`project:${p.id}`) })),
    ...series.map((m): Command => ({ label: `Meeting board: ${m.name}`, group: 'Go to', run: () => openBoard(`series:${m.id}`) })),
    { label: 'New note', group: 'Do', run: () => openNote(null) },
    { label: 'Wrap up the day', group: 'Do', words: 'end of day wrap-up', run: wrapUp },
    { label: 'Back up now', group: 'Do', words: 'backup save', run: backupNow },
    { label: 'Copy for stand-up', group: 'Do', words: 'yesterday standup clipboard', run: copyStandup },
    { label: 'Fit everything', group: 'Do', words: 'desk zoom', run: () => onDesk('fit') },
    { label: 'Tidy the Desk', group: 'Do', words: 'pile loose cards by project', run: () => onDesk('tidy') },
    { label: 'Timeline', group: 'Do', words: 'desk lay out', run: () => onDesk('timeline') },
    {
      label: app.justToday ? 'Show every card again' : 'Just today on the Desk',
      group: 'Do',
      words: 'fade focus today',
      run: () => {
        setView('desk');
        app.justToday = !app.justToday;
      },
    },
    ...(app.prefs?.timer
      ? [{ label: running.target ? 'Stop the timer' : 'Start the timer', group: 'Do' as const, words: 'clock', run: timerCommand }]
      : []),
    { label: 'Lock now', group: 'Do', keys: 'Ctrl L', run: () => onLock() },
    { label: 'Book today as holiday (8 h)', group: 'Do', words: 'leave vacation', run: () => holiday() },
    {
      label: 'Allow screenshots for 10 minutes',
      group: 'Do',
      run: () => attempt(allowCapture).then(() => toast('Screenshots and copying are allowed for 10 minutes.')),
    },
    {
      label: 'Protect now (block screenshots)',
      group: 'Do',
      run: () => attempt(protectNow).then(() => toast('Protected again.')),
    },
    ...SETTINGS.map((p): Command => ({ label: `Settings: ${p.label}`, group: 'Settings', words: p.words, run: () => settings(p.id) })),
  ]);

  /** Commands in their groups' order, keeping the ranking inside each group. */
  function grouped(list: Command[]): Command[] {
    return GROUPS.flatMap((g) => list.filter((c) => c.group === g));
  }

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let hits = $state<Hit[]>([]);
  let sel = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let box: HTMLElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | null = null;

  const logging = $derived(!!seen && seen.minutes !== null);

  // Phone (round 15, idea B): tap a time and a recent project instead of typing them.
  const TIMES = ['15m', '30m', '45m', '1h', '2h', '4h'];
  let pickedTime = $state('');
  let pickedProject = $state('');
  /** Projects you logged to lately, newest first (from the ribbon), then the rest. */
  const recentProjects = $derived.by(() => {
    const ids: string[] = [];
    for (const day of app.ribbon.flatMap((w) => w.days).reverse()) {
      for (const part of day.parts) if (part.projectId && !ids.includes(part.projectId)) ids.push(part.projectId);
    }
    return ids
      .map((id) => projectById(id))
      .filter((p) => !!p && !p.archived)
      .slice(0, 4) as NonNullable<ReturnType<typeof projectById>>[];
  });

  function swapWord(old: string, next: string): string {
    const words = text.split(/\s+/).filter((w) => w && w !== old);
    if (next) words.push(next);
    return words.join(' ') + ' ';
  }

  function pickTime(t: string) {
    const next = pickedTime === t ? '' : t;
    text = swapWord(pickedTime, next);
    pickedTime = next;
    read();
    input?.focus();
  }

  function pickProject(name: string) {
    const tag = '#' + name.replace(/\s+/g, '');
    const next = pickedProject === tag ? '' : tag;
    text = swapWord(pickedProject, next);
    pickedProject = next;
    read();
    input?.focus();
  }

  $effect(() => {
    if (app.captureOpen) {
      text = launch.prefill;
      launch.prefill = '';
      pickedTime = '';
      pickedProject = '';
      seen = null;
      hits = [];
      sel = 0;
      if (text) read();
      queueMicrotask(() => input?.focus());
      void api.meetingSeries().then((list) => (series = list), () => {});
    } else {
      launch.timerCapture = false;
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
    if (now.trim().toLowerCase() === 'undo') {
      seen = null;
      const list = [...changes.list].reverse();
      hits = list.map((change, after): Hit => ({ kind: 'change', change, after }));
      sel = 0;
      return;
    }
    if (now.trim().toLowerCase() === 'help') {
      seen = null;
      hits = grouped(commands).map((command): Hit => ({ kind: 'command', command }));
      sel = 0;
      return;
    }
    timer = setTimeout(async () => {
      const list = commands;
      const [c, found, day] = await Promise.all([
        api.parseCapture(now, app.today).catch(() => null),
        api
          .search(
            now,
            app.today,
            list.map((c) => (c.words ? `${c.label} ${c.words}` : c.label)),
          )
          .catch(() => null),
        api.findDay(now, app.today).catch(() => null),
      ]);
      if (text !== now) return;
      seen = c;
      const ranked = (found?.commands ?? []).flatMap((i) => list[i] ?? []).slice(0, MAX_COMMANDS);
      hits = [
        ...(day
          ? [
              { kind: 'day', date: day, sheet: false },
              { kind: 'day', date: day, sheet: true },
            ]
          : []),
        ...(found
          ? [
              ...found.entries.map((entry): Hit => ({ kind: 'entry', entry })),
              ...found.tickets.map((ticket): Hit => ({ kind: 'ticket', ticket })),
              ...found.notes.map((note): Hit => ({ kind: 'note', note })),
              ...found.inbox.map((item): Hit => ({ kind: 'scrap', item })),
            ]
          : []),
        ...grouped(ranked).map((command): Hit => ({ kind: 'command', command })),
      ] as Hit[];
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
        { date, minutes: 480, description: 'Holiday', projectId: null, recurringId: null, category: null, notes: null, timesheetCodeId: null },
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
    }, () => void attempt(() => api.restoreEntry(entry.id)).then(refresh));
  }

  async function toInbox(line: string) {
    const item = await attempt(() => api.addInbox(line));
    if (!item) return;
    close();
    await attempt(refreshInbox);
    toast('Saved to the Inbox', 'Undo', () => {
      void attempt(() => api.setInboxDeleted(item.id, true)).then(() => attempt(refreshInbox));
    }, () => void attempt(() => api.setInboxDeleted(item.id, false)).then(() => attempt(refreshInbox)));
  }

  async function open(hit: Hit) {
    close();
    if (hit.kind === 'command') {
      await hit.command.run();
    } else if (hit.kind === 'change') {
      undoBack(hit.change.id);
    } else if (hit.kind === 'day') {
      await selectDay(hit.date);
      setView(hit.sheet ? 'timesheet' : 'day');
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

  /** Phone: capture opened from "Start the timer": log the line with no time and time it. */
  async function startOn(line: string) {
    const parsed = seen ?? (await api.parseCapture(line, app.today).catch(() => null));
    const entry = await attempt(() => api.captureLog(parsed?.minutes == null ? `${line} 0m` : line, app.today));
    if (!entry) return;
    close();
    setView('desk');
    await selectDay(entry.date);
    await refresh();
    flashEntry(entry.id);
    if (entry.date === app.today) await startTimer(entry);
  }

  async function submit(toInboxAnyway: boolean) {
    const line = text.trim();
    if (!line) return;
    if (launch.timerCapture && !toInboxAnyway) {
      await startOn(line);
      return;
    }
    // "undo" and "help" are words for Find, not something to keep.
    if (!toInboxAnyway && hits.length === 0 && ['undo', 'help'].includes(line.toLowerCase())) return;
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
    }, () => void attempt(() => api.restoreEntry(entry.id)).then(refresh));
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
            : hit.kind === 'day'
              ? 'Days'
              : hit.kind === 'change'
                ? 'Recent changes, newest first'
              : hit.command.group;
  }
</script>

<div class="scrim" class:on={app.captureOpen} onclick={close} aria-hidden="true"></div>
<svelte:window onkeydown={(e) => app.captureOpen && trapTab(e, box)} />
<div
  bind:this={box}
  class="paper capture"
  class:on={app.captureOpen}
  class:sheet={phone.on}
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
    enterkeyhint="done"
    placeholder={launch.timerCapture
      ? 'What are you starting? e.g. printer INC0045678'
      : phone.on
      ? 'What did you do? e.g. printer INC0045678'
      : 'Log “1.5h printer INC0045678”, or find a ticket, a word or a command'}
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
    {:else if text.trim().toLowerCase() === 'undo' && hits.length === 0}
      <span class="none">Nothing to undo yet. Changes with an Undo button show here.</span>
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
          {:else if hit.kind === 'change'}
            <i class="dot cmd" aria-hidden="true">↶</i>
            <span class="what"><b>Undo: {hit.change.text}</b>
              <small>{agoText(hit.change.at, Date.now())}{hit.after ? ` · also undoes the ${hit.after === 1 ? 'one' : hit.after} after it` : ''}</small></span>
          {:else if hit.kind === 'day'}
            <i class="dot cmd" aria-hidden="true">›</i>
            {#if hit.sheet}
              {@const wk = ribbonWeekOf(hit.date)}
              <span class="what"><b>Timesheet, {wk ? `week of ${dayMonth(wk.weekStart)}` : 'that week'}</b>
                {#if wk}<small>{wk.hours} h logged</small>{/if}</span>
            {:else}
              {@const rd = ribbonDay(hit.date)}
              <span class="what"><b>Go to {longDay(hit.date)}</b><small>Day{rd ? ` · ${rd.hours} h` : ''}</small></span>
            {/if}
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
  {#if phone.on}
    <div class="picks">
      <span class="lbl">How long</span>
      <div class="row">
        {#each TIMES as t (t)}
          <button type="button" class="pick mono" aria-pressed={pickedTime === t} tabindex={app.captureOpen ? 0 : -1} onclick={() => pickTime(t)}>{t}</button>
        {/each}
      </div>
      {#if recentProjects.length}
        <span class="lbl">Recent</span>
        <div class="row">
          {#each recentProjects as p (p.id)}
            {@const tag = '#' + p.name.replace(/\s+/g, '')}
            <button type="button" class="pick" aria-pressed={pickedProject === tag} tabindex={app.captureOpen ? 0 : -1} onclick={() => pickProject(p.name)}>
              <i class="dot" style:--c={p.color ?? 'var(--none)'}></i>{p.name}
            </button>
          {/each}
        </div>
      {/if}
      <div class="row acts">
        <button type="button" class="btn" tabindex={app.captureOpen ? 0 : -1} disabled={!text.trim()} onclick={() => void submit(true)}>Save as scrap</button>
        <button type="button" class="btn primary" tabindex={app.captureOpen ? 0 : -1} disabled={!text.trim()} onclick={() => void submit(false)}>
          {launch.timerCapture
            ? 'Start the timer'
            : logging
              ? `Log ${fmtMinutes(seen?.minutes ?? 0)}`
              : hits.length
                ? 'Open'
                : 'Save'}
        </button>
      </div>
    </div>
  {:else}
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
      <span class="tip">Type “help” for everything Find can do</span>
    </div>
  {/if}
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

  /* Phone: a sheet from the bottom, above the keyboard. */
  .capture.sheet {
    top: auto;
    bottom: 0;
    left: 0;
    width: 100%;
    border-radius: 18px 18px 0 0;
    transform: translateY(100%);
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }

  .capture.sheet.on {
    transform: none;
  }

  .sheet .hits {
    max-height: 32vh;
  }

  .picks {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 14px 14px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .pick {
    min-height: 40px;
    padding: 0 14px;
    border-radius: 99px;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .pick[aria-pressed='true'] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  .pick .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }

  .acts {
    margin-top: 6px;
  }

  .acts .btn {
    flex: 1;
    min-height: 46px;
    justify-content: center;
    text-align: center;
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
