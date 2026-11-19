<script lang="ts">
  // A MOP on its change ticket (round 18, card r18-mop), inside its window. Along the top:
  // where it stands (Draft, Ready, Running, Done, Backed out) and its change window. Then Read
  // (the page as it reads), Edit (the text, saving as you type) or Run (tick each line off;
  // ticking a heading or a step crosses out everything under it, and each tick keeps its time).
  // Copy run log puts the steps with their times on the clipboard for the change record. The
  // rules are in crates/core/src/change.rs and crates/core/src/mop.rs.
  // Round 21 (card r21-mop-finish): Done asks first when steps aren't ticked; a finished run
  // offers to tick its ticket done, log how long it took and copy the run log; any step can
  // hold a "what I saw" line that goes into the run log.
  import { onDestroy, untrack } from 'svelte';
  import { tabIndent } from '../lib/indent';
  import { indentDepth } from '../lib/wikiText';
  import { api } from '../lib/api';
  import type { CheckLine, ItemSummary, MopRun, MopStatus, Note } from '../lib/api';
  import { refreshBell } from '../lib/bell.svelte';
  import { refreshMops } from '../lib/mops.svelte';
  import { focusNow } from '../lib/focus';
  import { fmtMinutes } from '../lib/format';
  import { localWords, runLog, stepsLeft, took } from '../lib/mopLog';
  import { copyText } from '../lib/protection.svelte';
  import { responseButton } from '../lib/responses';
  import { logTicketTime } from '../lib/trayActions';
  import { app, attempt, openWiki, refresh } from '../lib/store.svelte';
  import { goToTicket } from '../lib/ticketFind';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  import WikiLinks from './WikiLinks.svelte';
  import { saveAsTemplate } from '../lib/noteActions';
  import WikiText from './WikiText.svelte';

  let { id, ontitle }: { id: string; ontitle?: (title: string) => void } = $props();

  const SAVE_AFTER_MS = 700;
  const STEPS: { status: MopStatus; label: string }[] = [
    { status: 'draft', label: 'Draft' },
    { status: 'ready', label: 'Ready' },
    { status: 'running', label: 'Running' },
    { status: 'done', label: 'Done' },
  ];

  let note = $state<Note | null>(null);
  let run = $state<MopRun | null>(null);
  let mode = $state<'read' | 'edit' | 'run'>('read');
  let text = $state('');
  let titleText = $state('');
  let checks = $state<CheckLine[]>([]);
  let numbers = $state(true);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let windowEdit = $state<{ on: string; at: string } | null>(null);
  /** The ticket number typed for a MOP that isn't on a ticket yet. */
  let ticketIn = $state('');
  /** Done was pressed with this many steps not ticked: the strip asks first. */
  let asking = $state<number | null>(null);
  /** The run was just finished here: what's left to offer, and what has been done. */
  let closing = $state<{ item: ItemSummary | null; ticked: boolean; logged: boolean; copied: boolean } | null>(null);
  /** The step whose "what I saw" box is open (its tick key), and what's typed in it. */
  let seeing = $state<string | null>(null);
  let sawText = $state('');

  const lines = $derived(text.split('\n'));
  const tickable = $derived(checks.filter((c) => c.key).length);
  const ticked = $derived(checks.filter((c) => c.ticked).length);
  const at = (key: string) => run?.tickTimes.find((t) => t.key === key)?.at ?? null;
  const saw = (key: string) => run?.seen.find((s) => s.key === key)?.text ?? '';
  const statusLabel = (status: MopStatus) => STEPS.find((s) => s.status === status)?.label ?? 'Backed out';

  async function load() {
    const [n, r] = await Promise.all([attempt(() => api.getNote(id)), attempt(() => api.mopRun(id))]);
    if (!n) return;
    note = n;
    run = r ?? null;
    text = n.body;
    titleText = n.title;
    ontitle?.(n.title);
    mode = !run || (run.status === 'draft' && !n.body.trim()) ? 'edit' : run.status === 'running' ? 'run' : mode;
  }

  $effect(() => {
    void id;
    untrack(() => void load());
  });

  // The checklist (which lines are ticked and crossed out) comes from the app.
  $effect(() => {
    if (mode !== 'run' || !note) return;
    const body = text;
    const ticks = [...note.ticks];
    const t = setTimeout(async () => {
      const got = await attempt(() => api.mopChecklist(body, ticks));
      if (got) checks = got;
    }, 80);
    return () => clearTimeout(t);
  });

  function typed() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void flush(), SAVE_AFTER_MS);
  }

  async function flush() {
    if (timer) clearTimeout(timer);
    timer = null;
    const n = note;
    const title = titleText.trim() || n?.title || '';
    if (!n || (text === n.body && title === n.title)) return;
    const saved = await attempt(() =>
      api.updateNote(n.id, { kind: n.kind, title, body: text, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (saved && note?.id === saved.id) {
      note = { ...saved, body: text };
      if (saved.title !== n.title) {
        ontitle?.(saved.title);
        void refreshMops();
      }
    }
  }

  onDestroy(() => void flush());

  async function changed() {
    run = (await attempt(() => api.mopRun(id))) ?? run;
    void refreshMops();
    void refreshBell();
  }

  async function setStatus(status: MopStatus, sure = false) {
    if (!run || run.status === status) return;
    // Done with steps not ticked asks first.
    if (status === 'done' && !sure) {
      await flush();
      const got = mode === 'run' ? checks : ((await attempt(() => api.mopChecklist(text, note?.ticks ?? []))) ?? []);
      const left = stepsLeft(got);
      if (left > 0) {
        asking = left;
        return;
      }
    }
    asking = null;
    const before = run.status;
    if (!(await attempt(() => api.mopSetStatus(id, status).then(() => true)))) return;
    await changed();
    if (status === 'running') mode = 'run';
    // A finished run offers what closes it out: its ticket, the time, the run log.
    if (status === 'done' || status === 'backed_out') {
      const ticket = run.ticket;
      const item = (await attempt(() => api.items([ticket])))?.[0] ?? null;
      closing = { item, ticked: false, logged: false, copied: false };
    } else {
      closing = null;
    }
    toast(`MOP is ${statusLabel(status)}`, 'Undo', () =>
      void attempt(() => api.mopSetStatus(id, before)).then(() => {
        closing = null;
        return changed();
      }),
    );
  }

  async function tickTicketDone() {
    const item = closing?.item;
    if (!closing || !item) return;
    const set = async (done: boolean) => {
      if (!(await attempt(() => api.itemSetDone(item.id, done, app.today).then(() => true)))) return;
      if (closing) closing.ticked = done;
      await refresh();
    };
    await set(true);
    toast(`${item.number} is done`, 'Undo', () => void set(false), () => void set(true), responseButton(item.number));
  }

  async function logRunTime() {
    if (!closing || !run?.ranMinutes) return;
    closing.logged = true;
    await logTicketTime(run.ticket, run.ranMinutes);
  }

  function openSaw(key: string) {
    void saveSaw();
    sawText = saw(key);
    seeing = key;
  }

  async function saveSaw() {
    const key = seeing;
    if (!key) return;
    seeing = null;
    if (sawText.trimEnd() === saw(key)) return;
    const typed = sawText;
    if (await attempt(() => api.mopSetSeen(id, key, typed).then(() => true))) run = (await attempt(() => api.mopRun(id))) ?? run;
  }

  async function tick(i: number) {
    const key = checks[i]?.key;
    const n = note;
    if (!key || !n) return;
    await flush();
    const ticks = n.ticks.includes(key) ? n.ticks.filter((k) => k !== key) : [...n.ticks, key];
    note = { ...n, ticks };
    const back = await attempt(() => api.mopTick(id, ticks));
    if (back) note = { ...back, body: text };
    run = (await attempt(() => api.mopRun(id))) ?? run;
    // The first tick on a Ready MOP means it's running.
    if (run && (run.status === 'ready' || run.status === 'draft') && ticks.length) await setStatus('running');
  }

  async function clearTicks() {
    const n = note;
    const seen = run?.seen ?? [];
    if (!n || (!n.ticks.length && !seen.length)) return;
    const before = n.ticks;
    const back = await attempt(() => api.mopTick(id, []));
    if (!back) return;
    note = { ...back, body: text };
    // A fresh run starts without what was seen last time, too.
    for (const s of seen) await attempt(() => api.mopSetSeen(id, s.key, ''));
    run = (await attempt(() => api.mopRun(id))) ?? run;
    toast(seen.length ? 'Ticks and what you saw cleared for a fresh run' : 'Ticks cleared for a fresh run', 'Undo', () =>
      void attempt(() => api.mopTick(id, before)).then(async (b) => {
        if (b) note = { ...b, body: text };
        for (const s of seen) await attempt(() => api.mopSetSeen(id, s.key, s.text));
        run = (await attempt(() => api.mopRun(id))) ?? run;
      }),
    );
  }

  function clock(ts: string | null): string {
    if (!ts) return '';
    const d = new Date(ts);
    return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function hhmm(minutes: number): string {
    return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
  }

  function windowText(r: MopRun): string {
    if (!r.windowOn || r.windowAt === null) return 'Set the change window';
    const day = r.windowOn === app.today ? 'Today' : new Date(`${r.windowOn}T00:00`).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' });
    return `${day} ${hhmm(r.windowAt)}`;
  }

  async function saveWindow(clear = false) {
    const w = windowEdit;
    if (!w && !clear) return;
    const [h, m] = (w?.at ?? '').split(':').map(Number);
    const minutes = clear || h === undefined || m === undefined || Number.isNaN(h) || Number.isNaN(m) ? null : h * 60 + m;
    const on = clear || minutes === null ? null : w!.on;
    if (!(await attempt(() => api.mopSetWindow(id, on, minutes).then(() => true)))) return;
    windowEdit = null;
    await changed();
  }

  /** The run log for the change record: the day, start and finish, each step with when it
   *  was ticked, and what was seen (built in lib/mopLog.ts). */
  async function copyLog() {
    if (!note || !run) return;
    await saveSaw();
    let got = checks;
    if (mode !== 'run') got = (await attempt(() => api.mopChecklist(text, note!.ticks))) ?? [];
    if (!(await copyText(runLog(run, note.title, statusLabel(run.status), lines, got)))) return;
    if (closing) closing.copied = true;
    toast('Run log copied');
  }

  async function copyPlain() {
    if (await copyText(text)) toast('MOP copied as plain text');
  }

  /** Save a copy as a text file named after the MOP (what the Name box says). Like every
   *  export, the file is readable by anyone who can open it, and says so first. */
  async function saveFile() {
    const n = note;
    if (!n) return;
    await flush();
    toast("The file won't be locked: anyone who can open its folder can read it.");
    const name = await attempt(() => api.saveMopFile(n.id));
    if (name) toast(`Saved as ${name}`);
  }

  /** A MOP written before its ticket existed: put it on the ticket (made if new). */
  async function link() {
    const t = ticketIn.trim();
    if (!t) return;
    await flush();
    // A ticket on no day (made just now) goes on today's tray in the same step, so it
    // shows on the Desk and can be found again.
    const added = await attempt(() => api.mopLink(id, t, app.today));
    if (added === undefined) return;
    ticketIn = '';
    await changed();
    const number = run?.ticket ?? t;
    await refresh();
    toast(
      added ? `MOP is on ${number} now, and ${number} is on today's Tickets tray.` : `MOP is on ${number} now.`,
      'Go to the ticket',
      () => void goToTicket(number),
    );
  }

  async function follow(title: string) {
    const page = await attempt(() => api.wikiPageByTitle(title));
    if (page) openWiki(page);
    else toast(`No wiki page called “${title}” yet.`);
  }
</script>

{#snippet ticketLink(number: string)}
  <button type="button" class="linkish tk" title="Show {number} on the Tickets page" onclick={() => void goToTicket(number)}>{number}</button>
{/snippet}

{#if note}
  <div class="mop">
    <div class="top">
      {#if run}
      <div class="steps" role="group" aria-label="Where this MOP stands">
        {#each STEPS as s, i (s.status)}
          <button
            type="button"
            class:on={run.status === s.status}
            class:past={STEPS.findIndex((x) => x.status === run!.status) > i}
            aria-pressed={run.status === s.status}
            onclick={() => void setStatus(s.status)}>{s.label}</button
          >
        {/each}
        <button type="button" class="bad" class:on={run.status === 'backed_out'} aria-pressed={run.status === 'backed_out'} onclick={() => void setStatus('backed_out')}>
          Backed out
        </button>
      </div>
      {#if windowEdit}
        <form
          class="win"
          onsubmit={(e) => {
            e.preventDefault();
            void saveWindow();
          }}
        >
          <input class="text-in" type="date" bind:value={windowEdit.on} aria-label="Window day" />
          <input class="text-in" type="time" bind:value={windowEdit.at} aria-label="Window start" />
          <button type="submit" class="btn primary slim">Set</button>
          <button type="button" class="btn slim" onclick={() => void saveWindow(true)}>Clear</button>
        </form>
      {:else}
        <button
          type="button"
          class="btn slim when"
          title="When the change window starts: it shows in the bell and on the bottom bar"
          onclick={() => (windowEdit = { on: run!.windowOn ?? app.today, at: run!.windowAt !== null ? hhmm(run!.windowAt) : '22:00' })}
        >
          <Icon name="clock" size={13} />{windowText(run)}
        </button>
      {/if}
      {:else}
        <form
          class="link"
          onsubmit={(e) => {
            e.preventDefault();
            void link();
          }}
        >
          <span>Not on a ticket yet.</span>
          <input class="text-in" bind:value={ticketIn} placeholder="Ticket number, e.g. CHG0012345" aria-label="Ticket number" />
          <button type="submit" class="btn primary slim" disabled={!ticketIn.trim()}>Put it on the ticket</button>
        </form>
      {/if}
      <span class="grow"></span>
      <div class="modes" role="tablist" aria-label="How to show it">
        {#each run ? [['read', 'Read'], ['edit', 'Edit'], ['run', 'Run']] : [['read', 'Read'], ['edit', 'Edit']] as [m, label] (m)}
          <button
            type="button"
            role="tab"
            aria-selected={mode === m}
            onclick={() => {
              void flush();
              mode = m as typeof mode;
            }}>{label}</button
          >
        {/each}
      </div>
    </div>

    {#if !run}
      <p class="from">Write it now; it can go on its ticket once the ticket exists.</p>
    {:else if run.templateId}
      <p class="from">
        For {@render ticketLink(run.ticket)} · from a <button type="button" class="linkish" onclick={() => openWiki(run!.templateId)}>wiki template</button>
      </p>
    {:else}
      <p class="from">For {@render ticketLink(run.ticket)}</p>
    {/if}

    <!-- Its name, always in view (it used to be an unmarked box only Edit showed): type a new
         one and it's saved, and shows on the window, the bottom bar and in Find. -->
    <label class="name">
      <span>Name</span>
      <input class="text-in title-in" bind:value={titleText} oninput={typed} onblur={() => void flush()} placeholder="Give this MOP a name" autocomplete="off" />
    </label>

    {#if asking !== null}
      <div class="strip ask" role="group" aria-label="Steps not ticked">
        <span class="say"><b>{asking === 1 ? "1 step isn't ticked." : `${asking} steps aren't ticked.`}</b> Mark it done anyway?</span>
        <button type="button" class="btn slim" onclick={() => (asking = null)}>Keep going</button>
        <button type="button" class="btn slim primary" use:focusNow onclick={() => void setStatus('done', true)}>Mark it done</button>
      </div>
    {:else if closing && run && (run.status === 'done' || run.status === 'backed_out')}
      {@const long = took(run.startedAt, run.finishedAt)}
      <div class="strip" role="group" aria-label="Finish up">
        <span class="say">
          <b>{statusLabel(run.status)}{run.finishedAt ? ` at ${localWords.clock(run.finishedAt)}` : ''}{long ? `, after ${long}` : ''}.</b> Finish up:
        </span>
        {#if run.status === 'done' && closing.item && (!closing.item.done || closing.ticked)}
          <button type="button" class="btn slim" disabled={closing.ticked} onclick={() => void tickTicketDone()}>
            {#if closing.ticked}<Icon name="check" size={13} /> {run.ticket} is done{:else}Tick {run.ticket} done{/if}
          </button>
        {/if}
        {#if run.ranMinutes}
          <button type="button" class="btn slim" disabled={closing.logged} onclick={() => void logRunTime()}>
            {#if closing.logged}<Icon name="check" size={13} /> Time logged{:else}<Icon name="clock" size={13} /> Log {fmtMinutes(run.ranMinutes)}{/if}
          </button>
        {/if}
        <button type="button" class="btn slim" onclick={() => void copyLog()}>
          <Icon name={closing.copied ? 'check' : 'copy'} size={13} /> {closing.copied ? 'Run log copied' : 'Copy run log'}
        </button>
        <button type="button" class="shut" aria-label="Close" title="Close" onclick={() => (closing = null)}><Icon name="x" size={13} /></button>
      </div>
    {/if}

    <!-- Ctrl+F looks in here when the MOP is being read or run (in Edit, in the text box). -->
    <div class="body" data-find={mode === 'edit' ? undefined : ''}>
      {#if mode === 'edit'}
        <textarea use:tabIndent bind:value={text} oninput={typed} onblur={() => void flush()} aria-label="MOP text" spellcheck="true"></textarea>
      {:else if mode === 'read'}
        <WikiText body={text} onlink={(t) => void follow(t)} />
      {:else}
        <ol class="run" aria-label="Steps to tick off">
          {#each lines as line, i (i)}
            {@const c = checks[i]}
            <li
              class:x={c?.crossed}
              class:blank={!line.trim()}
              class:head={/^\s*#/.test(line)}
              style:padding-left={line.trim() ? `${4 + Math.min(8, indentDepth(line)) * 18}px` : undefined}
            >
              {#if numbers}<span class="no">{i + 1}</span>{/if}
              {#if c?.key}
                <button
                  type="button"
                  class="cb"
                  class:on={c.ticked}
                  class:under={c.crossed && !c.ticked}
                  aria-pressed={c.ticked}
                  aria-label="Tick line {i + 1}"
                  onclick={() => void tick(i)}>{#if c.ticked}<Icon name="check" size={11} />{/if}</button
                >
              {:else}
                <span class="cb none"></span>
              {/if}
              <span class="t">{line.trimStart().replace(/^#+\s*/, '')}</span>
              {#if c?.ticked}<span class="at">{clock(at(c.key))}</span>{/if}
              {#if c?.key && seeing !== c.key && !saw(c.key)}
                <button
                  type="button"
                  class="saw-add"
                  class:show={c.ticked}
                  aria-label="Write what you saw at line {i + 1}"
                  title="What I saw: paste output or a note. It goes into the run log."
                  onclick={() => openSaw(c.key)}><Icon name="note" size={13} /></button
                >
              {/if}
              {#if c?.key && seeing === c.key}
                <textarea
                  class="saw-in"
                  rows="3"
                  maxlength="4000"
                  spellcheck="false"
                  bind:value={sawText}
                  use:focusNow
                  aria-label="What you saw at line {i + 1}"
                  placeholder="What you saw: paste output or write a note. It goes into the run log."
                  onblur={() => void saveSaw()}
                  onkeydown={(e) => {
                    if (e.key === 'Escape') {
                      e.stopPropagation();
                      seeing = null;
                    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                      e.preventDefault();
                      void saveSaw();
                    }
                  }}
                ></textarea>
              {:else if c?.key && saw(c.key)}
                <button type="button" class="saw" title="What you saw here. Click to change it." onclick={() => openSaw(c.key)}>{saw(c.key)}</button>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    </div>
    <WikiLinks {text} />

    <footer>
      <span class="count">
        {[
          mode === 'run' || ticked ? `${ticked} of ${tickable} ticked` : '',
          run?.startedAt ? `started ${clock(run.startedAt)}` : '',
          run?.finishedAt ? `finished ${clock(run.finishedAt)}` : '',
        ]
          .filter(Boolean)
          .join(' · ')}
      </span>
      <span class="grow"></span>
      {#if mode === 'run'}
        <label class="nums"><input type="checkbox" bind:checked={numbers} /> Line numbers</label>
        <button type="button" class="btn slim" onclick={() => void clearTicks()}>Clear ticks</button>
      {/if}
      <button
        type="button"
        class="btn slim"
        title="Put a copy in the wiki's Templates, to use again on other tickets"
        onclick={() => void flush().then(() => note && saveAsTemplate({ ...note, body: text }))}>Save as template</button
      >
      <button type="button" class="btn slim" onclick={() => void copyPlain()}><Icon name="copy" size={13} /> Copy as text</button>
      <button
        type="button"
        class="btn slim"
        title="Save a copy as a text file, named after this MOP"
        onclick={() => void saveFile()}><Icon name="save" size={13} /> Save as a file</button
      >
      {#if run}<button type="button" class="btn slim primary" onclick={() => void copyLog()}><Icon name="copy" size={13} /> Copy run log</button>{/if}
    </footer>
  </div>
{/if}

<style>
  .mop {
    display: flex;
    flex-direction: column;
    gap: 6px;
    height: 100%;
    min-height: 0;
    padding: 8px 10px;
  }

  .link {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }

  .link .text-in {
    min-width: 14em;
  }

  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .grow {
    flex: 1;
  }

  .steps {
    display: inline-flex;
  }

  .steps button {
    border: 1px solid var(--rule);
    background: var(--paper);
    padding: 3px 9px;
    font: 600 12px var(--ui);
    color: var(--mut);
    cursor: pointer;
  }

  .steps button + button {
    border-left: 0;
  }

  .steps button:first-child {
    border-radius: 8px 0 0 8px;
  }

  .steps button:last-child {
    border-radius: 0 8px 8px 0;
  }

  .steps button.past {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 8%, var(--paper));
  }

  .steps button.on {
    background: var(--ok);
    color: #fff;
    border-color: var(--ok);
  }

  .steps button.bad {
    color: var(--bad, #b03a2e);
  }

  .steps button.bad.on {
    background: var(--bad, #b03a2e);
    border-color: var(--bad, #b03a2e);
    color: #fff;
  }

  .when {
    gap: 5px;
  }

  .win {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .modes {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }

  .modes button {
    border: 0;
    background: none;
    border-radius: 6px;
    padding: 3px 10px;
    font: 600 12px var(--ui);
    color: var(--mut);
    cursor: pointer;
  }

  .modes button[aria-selected='true'] {
    background: var(--paper);
    color: var(--ink);
    box-shadow: var(--shadow);
  }

  .from {
    margin: 0;
    font-size: 12px;
    color: var(--mut);
  }

  .linkish {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline dashed;
  }

  .linkish.tk {
    font-weight: 700;
    font-family: var(--mono);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .name span {
    flex: none;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .title-in {
    flex: 1;
    min-width: 0;
    font: 600 15px var(--ui);
  }

  textarea {
    width: 100%;
    height: 100%;
    min-height: 220px;
    resize: none;
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 8px 10px;
    font: 13.5px/1.5 var(--mono);
    background: var(--paper);
    color: var(--ink);
  }

  .run {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 13.5px;
  }

  .run li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 2px 4px;
    border-radius: 5px;
  }

  /* What I saw (round 21): a line of its own under the step, fixed-width so pasted output
     keeps its columns. */
  .run li {
    flex-wrap: wrap;
  }

  .saw,
  .saw-in {
    flex: 1 1 100%;
    margin: 2px 0 4px 26px;
    padding: 5px 8px;
    border-radius: 6px;
    font: 12px/1.45 var(--mono);
    color: var(--ink);
    white-space: pre;
    overflow-x: auto;
    tab-size: 4;
    text-align: left;
  }

  .saw {
    border: 0;
    border-left: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    border-radius: 0 6px 6px 0;
    background: color-mix(in srgb, var(--ink) 5%, transparent);
    cursor: text;
  }

  .run textarea.saw-in {
    width: auto;
    height: auto;
    min-height: 0;
    resize: vertical;
    border: 1px solid var(--accent);
    background: var(--paper);
  }

  .saw-add {
    flex: none;
    border: 0;
    background: none;
    padding: 0 2px;
    color: var(--mut);
    cursor: pointer;
    opacity: 0;
    position: relative;
    top: 2px;
  }

  .saw-add.show,
  .run li:hover .saw-add,
  .saw-add:focus-visible {
    opacity: 1;
  }

  @media (hover: none) {
    .saw-add {
      opacity: 1;
    }
  }

  /* Asking before Done, and what a finished run offers (round 21). */
  .strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 8px;
    font-size: 12.5px;
    background: color-mix(in srgb, var(--ok) 10%, var(--paper));
    border: 1px solid color-mix(in srgb, var(--ok) 35%, transparent);
  }

  .strip.ask {
    background: color-mix(in srgb, var(--warn, #b7791f) 12%, var(--paper));
    border-color: color-mix(in srgb, var(--warn, #b7791f) 45%, transparent);
  }

  .strip .say {
    flex: 1 1 14em;
  }

  .strip .btn {
    gap: 4px;
  }

  .shut {
    border: 0;
    background: none;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: var(--mut);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .run li.blank {
    min-height: 10px;
  }

  .run li.head .t {
    font: 600 14.5px var(--display);
  }

  .run li.x .t {
    text-decoration: line-through;
    color: var(--mut);
  }

  .no {
    width: 2.2em;
    text-align: right;
    font: 11px var(--mono);
    color: var(--mut);
    flex: none;
  }

  .cb {
    width: 16px;
    height: 16px;
    flex: none;
    border: 1.5px solid var(--mut);
    border-radius: 4px;
    background: var(--paper);
    display: grid;
    place-items: center;
    color: #fff;
    cursor: pointer;
    padding: 0;
    position: relative;
    top: 3px;
  }

  .cb.on {
    background: var(--ok);
    border-color: var(--ok);
  }

  .cb.under {
    border-color: color-mix(in srgb, var(--ok) 60%, transparent);
  }

  .cb.none {
    border-color: transparent;
    background: none;
    cursor: default;
  }

  .t {
    flex: 1;
    white-space: pre-wrap;
    tab-size: 4;
  }

  .at {
    font: 11.5px var(--mono);
    color: var(--ok);
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    border-top: 1px solid var(--rule);
    padding-top: 6px;
    font-size: 12px;
    color: var(--mut);
  }

  .nums {
    display: flex;
    gap: 4px;
    align-items: center;
  }
</style>
