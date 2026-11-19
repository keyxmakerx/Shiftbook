<script lang="ts">
  // Log work: paste a ticket or say what you did, then press a time button. The app
  // reuses today's entry for a ticket it already has (see crates/core/src/quicklog.rs).
  import { tick, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { Capture, SameWork } from '../lib/api';
  import type { Entry } from '../lib/api';
  import { addDays, fmtMinutes, weekdayLong } from '../lib/format';
  import { fillTimes, isManyLines, manyRows, missingTimes, oneLine, totalMinutes, type ManyRow } from '../lib/manyLines';
  import { repeatCandidates } from '../lib/repeat';
  import { app, attempt, flashEntry, projectById, projectColor, refresh, timeButtons, words } from '../lib/store.svelte';
  import { hasHome, hasWork } from '../lib/words';
  import { toast } from '../lib/toast.svelte';
  import { refreshRepeating, repeating } from '../lib/repeating.svelte';
  import Icon from './Icon.svelte';
  import KnownTickets from './KnownTickets.svelte';

  const TIMES = $derived(timeButtons());

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let input: HTMLInputElement | undefined = $state();
  let working = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let timesEl: HTMLElement | undefined = $state();
  let earlier = $state<Entry[]>([]);
  // "Add to …?" (round 15): the question waiting for an answer, and answers already given
  // this session (by title), so it is asked once.
  let ask = $state<{ minutes: number; line: string; same: SameWork } | null>(null);
  const answered = new Map<string, boolean>();
  /** The Desk spot of the scrap being logged: the entry it makes takes its place. */
  let spot: { x: number; y: number } | null = null;

  // The week before the selected day, for "Same as yesterday".
  $effect(() => {
    const day = app.selected;
    let live = true;
    api
      .listEntries(addDays(day, -7), addDays(day, -1))
      .then((list) => {
        if (live) earlier = list;
      })
      .catch(() => {
        if (live) earlier = [];
      });
    return () => {
      live = false;
    };
  });

  // Lines filed under a repeating thing (#name) show its name.
  $effect(() => {
    if (seen?.recurringId && repeating.items.length === 0) void refreshRepeating();
  });

  // Home and Both (round 15): a Done button saves the line with no time; time stays optional.
  const home = $derived(hasHome(app.prefs?.useMode));
  const workToo = $derived(hasWork(app.prefs?.useMode));

  const repeats = $derived(repeatCandidates(app.selected, earlier, app.entries));
  // The repeats come from the last day with work, which on a Monday is usually Friday.
  const repeatDay = $derived(repeats[0] && repeats[0].date !== addDays(app.selected, -1) ? weekdayLong(repeats[0].date) : 'yesterday');

  /** What to do once a line handed over with its time (the Now card) has been logged. */
  let afterLog: (() => void) | null = null;

  $effect(() => {
    const serial = app.logFocus.serial;
    if (serial === 0) return;
    untrack(() => {
      spot = app.logFocus.spot;
      if (app.logFocus.text) {
        text = app.logFocus.text;
        recognise();
      }
      // A line that comes with its time is logged straight away, once (the request is used
      // up, so opening Day or the Desk later doesn't log it again).
      const minutes = app.logFocus.minutes;
      afterLog = app.logFocus.text ? app.logFocus.then : null;
      app.logFocus.then = null;
      if (minutes !== null && app.logFocus.text) {
        app.logFocus.minutes = null;
        void log(minutes, app.logFocus.text);
        return;
      }
      queueMicrotask(() => input?.focus());
    });
  });

  function recognise() {
    ask = null;
    if (timer !== null) clearTimeout(timer);
    const now = text;
    if (!now.trim()) {
      seen = null;
      spot = null;
      // The line that was handed over is gone, so nothing waits on it any more.
      afterLog = null;
      return;
    }
    timer = setTimeout(async () => {
      const c = await api.parseCapture(now, app.selected).catch(() => null);
      if (text === now) seen = c;
    }, 120);
  }

  // Several lines pasted at once (round 21): a preview, one entry per line.
  let many = $state<{ rows: ManyRow[]; leftOut: number; pasted: string } | null>(null);
  let manyEl: HTMLElement | undefined = $state();

  /** A paste of several lines opens the preview; one line pastes as it always did. */
  async function pasted(e: ClipboardEvent) {
    const got = e.clipboardData?.getData('text/plain') ?? '';
    if (!isManyLines(got)) return;
    e.preventDefault();
    const read = await attempt(() => api.captureLines(got, app.selected));
    if (!read) return;
    if (read.lines.length < 2) {
      asOne(got);
      return;
    }
    ask = null;
    many = { rows: manyRows(read.lines), leftOut: read.leftOut, pasted: got };
    await tick();
    manyEl?.querySelector<HTMLElement>('select.need, .logall')?.focus();
  }

  /** "One entry instead": the pasted text as the one line the box used to make of it. */
  function asOne(got: string) {
    many = null;
    text = oneLine(got);
    recognise();
    input?.focus();
  }

  function setRow(i: number, minutes: number | null) {
    if (!many) return;
    many = { ...many, rows: many.rows.map((r, n) => (n === i ? { ...r, minutes } : r)) };
  }

  function dropRow(i: number) {
    if (!many) return;
    const rows = many.rows.filter((_, n) => n !== i);
    many = rows.length ? { ...many, rows } : null;
    if (!many) input?.focus();
  }

  /** The times a row can pick: the time buttons, and the time its own line had. */
  function rowTimes(row: ManyRow): number[] {
    const all = new Set(TIMES);
    if (row.minutes !== null && row.minutes > 0) all.add(row.minutes);
    return [...all].sort((a, b) => a - b);
  }

  /** Log every line in the preview, in order. A line that fails stays, with the ones after it. */
  async function logMany() {
    const m = many;
    if (!m) return;
    working = true;
    const before = new Set(app.entries.map((e) => e.id));
    const done: { id: string; existed: boolean; minutes: number }[] = [];
    for (const row of m.rows) {
      const minutes = row.minutes ?? 0;
      const entry = await attempt(() => api.quickLog(app.selected, row.text, minutes, null));
      if (!entry) break;
      done.push({ id: entry.id, existed: before.has(entry.id), minutes });
      before.add(entry.id);
    }
    working = false;
    const left = m.rows.slice(done.length);
    many = left.length ? { ...m, rows: left } : null;
    if (done.length === 0) return;
    await refresh();
    flashEntry(done[done.length - 1]!.id);
    if (!many) input?.focus();
    const total = done.reduce((sum, d) => sum + d.minutes, 0);
    toast(`Logged ${done.length} ${done.length === 1 ? 'entry' : 'entries'}${total ? ` · ${fmtMinutes(total)}` : ''}`, 'Undo', () => {
      void attempt(async () => {
        // Newest first, so time added to an entry made here comes off before the entry goes.
        for (const d of [...done].reverse()) {
          if (!d.existed) await api.deleteEntry(d.id);
          else if (d.minutes) await api.addEntryMinutes(d.id, -d.minutes);
        }
        return true;
      }).then(refresh);
    });
  }

  /** Enter: log the line when it has a time in it, otherwise move to the time buttons. */
  async function enter() {
    if (!text.trim()) return;
    const now = text;
    const c = await api.parseCapture(now, app.selected).catch(() => null);
    if (text !== now) return;
    seen = c;
    if (c?.minutes && c.hoursText) {
      await log(c.minutes, now.replace(c.hoursText, ' ').replace(/\s+/g, ' ').trim());
    } else {
      timesEl?.querySelector<HTMLButtonElement>('button')?.focus();
    }
  }

  /** Left and right arrows move between the time buttons. */
  function timesKey(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    const list = [...(timesEl?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    event.preventDefault();
    if (event.key === 'ArrowLeft' && i === 0) input?.focus();
    else list[Math.min(list.length - 1, Math.max(0, i + (event.key === 'ArrowRight' ? 1 : -1)))]?.focus();
  }

  async function repeat(e: Entry) {
    working = true;
    const entry = await attempt(() =>
      api.createEntry(
        {
          date: app.selected,
          minutes: e.minutes,
          description: e.description,
          projectId: e.projectId,
          recurringId: e.recurringId,
          category: e.category,
          notes: null,
          timesheetCodeId: e.timesheetCodeId,
        },
        '',
      ),
    );
    working = false;
    if (!entry) return;
    await refresh();
    flashEntry(entry.id);
    toast(`Logged ${fmtMinutes(e.minutes)} of ${e.description}`, 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    }, () => void attempt(() => api.restoreEntry(entry.id)).then(refresh));
  }

  async function log(minutes: number, line = text, addTo?: boolean) {
    if (!line.trim()) {
      input?.focus();
      toast('Paste a ticket or type what you did first.');
      return;
    }
    working = true;
    if (addTo === undefined) {
      const same = await api.sameWork(app.selected, line).catch(() => null);
      if (same) {
        const before = answered.get(same.title.toLowerCase());
        if (before === undefined) {
          working = false;
          ask = { minutes, line, same };
          return;
        }
        addTo = before;
      }
    }
    ask = null;
    const before = new Map(app.entries.map((e) => [e.id, e.minutes]));
    const entry = await attempt(() => api.quickLog(app.selected, line, minutes, addTo ?? null));
    working = false;
    if (!entry) return;
    const existed = before.has(entry.id);
    // A new entry from a scrap on the Desk shows where the scrap was.
    if (spot && !existed) {
      const at = spot;
      await attempt(() => api.canvasMove('entry', entry.id, at.x, at.y));
    }
    spot = null;
    text = '';
    seen = null;
    const after = afterLog;
    afterLog = null;
    await refresh();
    flashEntry(entry.id);
    after?.();
    toast(
      minutes === 0
        ? existed
          ? `${entry.description} is already down for today`
          : `Done: ${entry.description}`
        : existed
        ? `Added ${fmtMinutes(minutes)} to ${entry.description}`
        : `Logged ${fmtMinutes(minutes)}${entry.tickets.length ? ' on ' + entry.tickets.map((t) => t.number).join(', ') : ''}`,
      'Undo',
      () => {
        const undo: Promise<unknown> = existed ? api.addEntryMinutes(entry.id, -minutes) : api.deleteEntry(entry.id);
        void attempt(() => undo).then(refresh);
      },
    );
  }
</script>

<div class="quick">
  <label class="lbl" for="log-work">
    {#if home && !workToo}
      {words().logWork} · type what you did and press Done; a time is optional
    {:else if home}
      Log work · say what you did and press Done, or paste a ticket and pick a time
    {:else}
      Log work · paste a ticket or say what you did, add a time or pick one
    {/if}
  </label>
  <div class="in">
    <input
      id="log-work"
      class="text-in"
      bind:this={input}
      bind:value={text}
      oninput={recognise}
      onpaste={(e) => void pasted(e)}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          void enter();
        } else if (e.key === 'ArrowRight' && input && input.selectionStart === text.length && text) {
          e.preventDefault();
          timesEl?.querySelector<HTMLButtonElement>('button')?.focus();
        }
      }}
      placeholder={home && !workToo ? 'e.g. “paid the water bill”' : 'e.g. “patched print server 45m”'}
      autocomplete="off"
      spellcheck="false"
    />
    <span class="times" role="toolbar" tabindex="-1" aria-label="Log it as" bind:this={timesEl} onkeydown={timesKey}>
      {#if home}
        <button type="button" class="hbtn donebtn" disabled={working} onclick={() => log(0)} title="Save it with no time">✓ Done</button>
      {/if}
      {#each TIMES as m (m)}
        <button type="button" class="hbtn" disabled={working} onclick={() => log(m)}>{fmtMinutes(m)}</button>
      {/each}
    </span>
  </div>
  <div class="recog" aria-live="polite">
    {#if seen && text.trim()}
      {#each seen.tickets as t (t)}
        <span class="chip" style:--c={projectColor(seen.projectId)}>{t}</span>
      {/each}
      {#if seen.projectId}
        <span>goes to <b>{projectById(seen.projectId)?.name}</b></span>
      {:else if seen.recurringId}
        {@const id = seen.recurringId}
        <span>goes to <b class="rep-name"><Icon name="repeat" size={12} /> {repeating.items.find((r) => r.id === id)?.name ?? 'a repeating thing'}</b></span>
      {:else if seen.projectQuery}
        <span class="warn">nothing called “{seen.projectQuery}”</span>
      {/if}
      {#if seen.description}<span>as “{seen.description}”</span>{/if}
      {#if seen.minutes}
        <span class="lbl pick"><b class="mono">{fmtMinutes(seen.minutes)}</b> · <span class="kbd">Enter</span> logs it</span>
      {:else}
        <span class="lbl pick">{home ? 'press Done, or pick a time' : 'pick a time'}, or <span class="kbd">Enter</span> to jump to them</span>
      {/if}
    {/if}
  </div>
  {#if seen && text.trim() && seen.tickets.length > 0}
    <KnownTickets numbers={seen.tickets} />
  {/if}
  {#if many}
    {@const m = many}
    {@const missing = missingTimes(m.rows)}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="many"
      role="group"
      aria-label="Several lines to log"
      bind:this={manyEl}
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          many = null;
          input?.focus();
        }
      }}
    >
      <div class="mhead">
        <b>{m.rows.length} {m.rows.length === 1 ? 'entry' : 'entries'} from {m.rows.length} {m.rows.length === 1 ? 'line' : 'lines'}</b>
        <small>
          {[
            totalMinutes(m.rows) ? `${fmtMinutes(totalMinutes(m.rows))} in all` : '',
            missing ? `${missing} ${missing === 1 ? 'line has' : 'lines have'} no time yet` : '',
            m.leftOut ? `${m.leftOut} more ${m.leftOut === 1 ? 'line was' : 'lines were'} left out (20 at a time)` : '',
          ]
            .filter(Boolean)
            .join(' · ')}
        </small>
      </div>
      <ul>
        {#each m.rows as row, i (i)}
          <li>
            <span class="what" title={row.text}>
              {row.text}
              {#if row.projectId}<small style:--c={projectColor(row.projectId)}>{projectById(row.projectId)?.name}</small>{/if}
            </span>
            <select
              class="text-in"
              class:need={row.minutes === null}
              aria-label="Time for {row.text}"
              value={row.minutes === null ? '' : String(row.minutes)}
              onchange={(e) => {
                const v = (e.currentTarget as HTMLSelectElement).value;
                setRow(i, v === '' ? null : Number(v));
              }}
            >
              <option value="">{home ? 'no time' : 'pick a time'}</option>
              {#each rowTimes(row) as t (t)}
                <option value={String(t)}>{fmtMinutes(t)}</option>
              {/each}
            </select>
            <button type="button" class="drop" aria-label="Leave out: {row.text}" title="Leave this line out" onclick={() => dropRow(i)}>
              <Icon name="x" size={13} />
            </button>
          </li>
        {/each}
      </ul>
      {#if missing}
        <div class="fill">
          <span class="lbl">Lines with no time</span>
          {#each TIMES as t (t)}
            <button type="button" class="hbtn" onclick={() => (many = { ...m, rows: fillTimes(m.rows, t) })}>{fmtMinutes(t)}</button>
          {/each}
        </div>
      {/if}
      <div class="mbtns">
        <button type="button" class="btn primary logall" disabled={working || (missing > 0 && !home)} onclick={() => void logMany()}>
          Log {m.rows.length === 1 ? 'it' : `all ${m.rows.length}`}
        </button>
        <button type="button" class="btn" onclick={() => asOne(m.pasted)}>One entry instead</button>
        <button
          type="button"
          class="btn"
          onclick={() => {
            many = null;
            input?.focus();
          }}>Cancel</button
        >
      </div>
    </div>
  {/if}
  {#if ask}
    {@const a = ask}
    {@const home = projectById(a.same.projectId)?.name ?? repeating.items.find((r) => r.id === a.same.recurringId)?.name}
    <div class="ask" role="group" aria-label="Add to earlier work?">
      <span class="q">
        <b>Add to “{a.same.title}”?</b>
        <small>
          {[
            home,
            `${fmtMinutes(a.same.minutes)} over ${a.same.days} ${a.same.days === 1 ? 'day' : 'days'}`,
            a.same.openTickets.length ? `${a.same.openTickets.join(', ')} still open` : '',
          ]
            .filter(Boolean)
            .join(' · ')}
        </small>
      </span>
      <button
        type="button"
        class="btn primary"
        onclick={() => {
          answered.set(a.same.title.toLowerCase(), true);
          void log(a.minutes, a.line, true);
        }}>Yes, add to it</button
      >
      <button
        type="button"
        class="btn"
        onclick={() => {
          answered.set(a.same.title.toLowerCase(), false);
          void log(a.minutes, a.line, false);
        }}>No, it's new</button
      >
    </div>
  {/if}
  {#if repeats.length > 0}
    <div class="repeat">
      <span class="lbl">Same as {repeatDay}</span>
      {#each repeats as r (r.id)}
        <button type="button" class="btn rep" disabled={working} onclick={() => repeat(r)} title="Log {fmtMinutes(r.minutes)} of {r.description} again">
          <span class="chip" style:--c={projectColor(r.projectId)}>{fmtMinutes(r.minutes)}</span>{r.description}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .many {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border-radius: 8px;
    background: var(--paper);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .mhead {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    align-items: baseline;
  }

  .mhead small {
    color: var(--ink-2);
  }

  .many ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 260px;
    overflow: auto;
  }

  .many li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 6px;
    align-items: center;
  }

  .what {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13.5px;
  }

  .what small {
    margin-left: 6px;
    color: var(--c, var(--ink-2));
    font-weight: 600;
  }

  .many select {
    min-width: 128px;
    padding: 4px 6px;
    font: 600 12.5px var(--mono);
  }

  .many select.need {
    border-color: var(--warn);
    color: var(--warn);
  }

  .drop {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--mut);
    cursor: pointer;
  }

  .drop:hover {
    color: var(--ink);
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .fill,
  .mbtns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--paper);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .ask .q {
    flex: 1 1 220px;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .ask small {
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }

  .rep-name {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--rep);
  }

  .quick {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .in {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .in input {
    flex: 1 1 220px;
    font-size: 14.5px;
    padding: 8px 11px;
  }

  .recog {
    font-size: 12.5px;
    color: var(--ink-2);
    min-height: 20px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .donebtn {
    font-weight: 700;
    color: var(--ok);
  }

  .pick {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
  }

  .times {
    display: inline-flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .repeat {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .repeat .lbl {
    margin-right: 2px;
  }

  .rep {
    font-weight: 500;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warn {
    color: var(--warn);
  }
</style>
