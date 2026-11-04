<script lang="ts">
  // A MOP on its change ticket (round 18, card r18-mop), inside its window. Along the top:
  // where it stands (Draft, Ready, Running, Done, Backed out) and its change window. Then Read
  // (the page as it reads), Edit (the text, saving as you type) or Run (tick each line off;
  // ticking a heading or a step crosses out everything under it, and each tick keeps its time).
  // Copy run log puts the steps with their times on the clipboard for the change record. The
  // rules are in crates/core/src/change.rs and crates/core/src/mop.rs.
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { CheckLine, MopRun, MopStatus, Note } from '../lib/api';
  import { refreshBell } from '../lib/bell.svelte';
  import { refreshMops } from '../lib/mops.svelte';
  import { copyText } from '../lib/protection.svelte';
  import { app, attempt, openWiki } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  import WikiLinks from './WikiLinks.svelte';
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
  let checks = $state<CheckLine[]>([]);
  let numbers = $state(true);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let windowEdit = $state<{ on: string; at: string } | null>(null);

  const lines = $derived(text.split('\n'));
  const tickable = $derived(checks.filter((c) => c.key).length);
  const ticked = $derived(checks.filter((c) => c.ticked).length);
  const at = (key: string) => run?.tickTimes.find((t) => t.key === key)?.at ?? null;

  async function load() {
    const [n, r] = await Promise.all([attempt(() => api.getNote(id)), attempt(() => api.mopRun(id))]);
    if (!n) return;
    note = n;
    run = r ?? null;
    text = n.body;
    ontitle?.(n.title);
    mode = run?.status === 'running' ? 'run' : run?.status === 'draft' && !n.body.trim() ? 'edit' : mode;
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
    if (!n || text === n.body) return;
    const saved = await attempt(() =>
      api.updateNote(n.id, { kind: n.kind, title: n.title, body: text, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (saved && note?.id === saved.id) note = { ...saved, body: text };
  }

  onDestroy(() => void flush());

  async function changed() {
    run = (await attempt(() => api.mopRun(id))) ?? run;
    void refreshMops();
    void refreshBell();
  }

  async function setStatus(status: MopStatus) {
    if (!run || run.status === status) return;
    const before = run.status;
    if (!(await attempt(() => api.mopSetStatus(id, status).then(() => true)))) return;
    await changed();
    if (status === 'running') mode = 'run';
    toast(`MOP is ${STEPS.find((s) => s.status === status)?.label ?? 'Backed out'}`, 'Undo', () =>
      void attempt(() => api.mopSetStatus(id, before)).then(changed),
    );
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
    if (!n || !n.ticks.length) return;
    const before = n.ticks;
    const back = await attempt(() => api.mopTick(id, []));
    if (!back) return;
    note = { ...back, body: text };
    run = (await attempt(() => api.mopRun(id))) ?? run;
    toast('Ticks cleared for a fresh run', 'Undo', () =>
      void attempt(() => api.mopTick(id, before)).then(async (b) => {
        if (b) note = { ...b, body: text };
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

  /** The run log for the change record: each step with when it was ticked. */
  async function copyLog() {
    if (!note || !run) return;
    let got = checks;
    if (mode !== 'run') got = (await attempt(() => api.mopChecklist(text, note!.ticks))) ?? [];
    const out = [`${run.ticket} · ${note.title}`, `Status: ${STEPS.find((s) => s.status === run!.status)?.label ?? 'Backed out'}${run.startedAt ? ` · started ${clock(run.startedAt)}` : ''}`, ''];
    lines.forEach((line, i) => {
      const c = got[i];
      if (!line.trim()) return;
      const when = c?.ticked ? clock(at(c.key)) : '';
      const mark = c?.ticked ? '✓' : c?.crossed ? '–' : '☐';
      out.push(`${when.padEnd(5)} ${mark} ${line.trim()}`);
    });
    if (await copyText(out.join('\n'))) toast('Run log copied');
  }

  async function copyPlain() {
    if (await copyText(text)) toast('MOP copied as plain text');
  }

  async function follow(title: string) {
    const page = await attempt(() => api.wikiPageByTitle(title));
    if (page) openWiki(page);
    else toast(`No wiki page called “${title}” yet.`);
  }
</script>

{#if note && run}
  <div class="mop">
    <div class="top">
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
      <span class="grow"></span>
      <div class="modes" role="tablist" aria-label="How to show it">
        {#each [['read', 'Read'], ['edit', 'Edit'], ['run', 'Run']] as [m, label] (m)}
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

    {#if run.templateId}
      <p class="from">
        For <b>{run.ticket}</b> · from a <button type="button" class="linkish" onclick={() => openWiki(run!.templateId)}>wiki template</button>
      </p>
    {:else}
      <p class="from">For <b>{run.ticket}</b></p>
    {/if}

    <div class="body">
      {#if mode === 'edit'}
        <textarea bind:value={text} oninput={typed} onblur={() => void flush()} aria-label="MOP text" spellcheck="true"></textarea>
      {:else if mode === 'read'}
        <WikiText body={text} onlink={(t) => void follow(t)} />
      {:else}
        <ol class="run" aria-label="Steps to tick off">
          {#each lines as line, i (i)}
            {@const c = checks[i]}
            <li class:x={c?.crossed} class:blank={!line.trim()} class:head={/^\s*#/.test(line)}>
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
              <span class="t">{line.replace(/^\s*#+\s*/, '')}</span>
              {#if c?.ticked}<span class="at">{clock(at(c.key))}</span>{/if}
            </li>
          {/each}
        </ol>
      {/if}
    </div>
    <WikiLinks {text} />

    <footer>
      <span class="count">
        {#if mode === 'run' || ticked}{ticked} of {tickable} ticked{/if}{#if run.startedAt}{' · started '}{clock(run.startedAt)}{/if}
      </span>
      <span class="grow"></span>
      {#if mode === 'run'}
        <label class="nums"><input type="checkbox" bind:checked={numbers} /> Line numbers</label>
        <button type="button" class="btn slim" onclick={() => void clearTicks()}>Clear ticks</button>
      {/if}
      <button type="button" class="btn slim" onclick={() => void copyPlain()}><Icon name="copy" size={13} /> Copy as text</button>
      <button type="button" class="btn slim primary" onclick={() => void copyLog()}><Icon name="copy" size={13} /> Copy run log</button>
    </footer>
  </div>
{:else if note}
  <p class="none">This MOP isn't on a ticket.</p>
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

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
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

  .none {
    padding: 10px;
    color: var(--mut);
  }
</style>
