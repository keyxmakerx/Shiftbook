<script lang="ts">
  // The timesheet crib sheet: "what to enter" for a week. Everything here (grouping,
  // comments, totals, warnings, the copy text) comes ready-made from the app
  // (crates/core/src/sheet.rs); this page shows it and remembers the tick-offs.
  import { api } from '../lib/api';
  import type { GroupBy, SheetDay, SheetWarning, Tick, WeekSheet } from '../lib/api';
  import { addDays, dayMonth, dayOfMonth, weekdayLong, weekdayShort } from '../lib/format';
  import { copyText } from '../lib/protection.svelte';
  import { app, attempt, projectColor } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from '../components/Icon.svelte';

  let sheet = $state<WeekSheet | null>(null);
  let ticks = $state<Tick[]>([]);
  let groupBy = $state<GroupBy | null>(null);
  let anchor = $state(app.selected);
  let shut = $state<Record<string, boolean>>({});

  async function load() {
    const g = groupBy ?? app.prefs?.groupBy ?? 'code';
    const s = await attempt(() => api.weekSheet(anchor, app.today, g));
    if (!s) return;
    sheet = s;
    ticks = (await attempt(() => api.listTicks(s.weekStart))) ?? [];
  }

  $effect(() => {
    void anchor;
    void groupBy;
    void app.ribbon;
    void load();
  });

  $effect(() => {
    anchor = app.selected;
  });

  function done(day: string, key: string): boolean {
    return ticks.some((t) => t.day === day && t.lineKey === key && t.done);
  }

  function doneCount(d: SheetDay): number {
    return d.lines.filter((l) => done(d.date, l.key)).length;
  }

  function allDone(d: SheetDay): boolean {
    return d.lines.length > 0 && doneCount(d) === d.lines.length;
  }

  async function tick(d: SheetDay, key: string, value: boolean) {
    if (!sheet) return;
    const week = sheet.weekStart;
    const wasAll = allDone(d);
    const others = ticks.filter((t) => !(t.day === d.date && t.lineKey === key));
    ticks = [...others, { day: d.date, lineKey: key, done: value }];
    await attempt(() => api.setTick(week, d.date, key, value));
    if (!wasAll && allDone(d)) {
      toast(`${weekdayLong(d.date)} is entered. Nice.`);
      setTimeout(() => (shut[d.date] = true), 450);
    }
  }

  async function copy(text: string, ok: string) {
    toast((await copyText(text)) ? ok : "Couldn't copy here.");
  }

  function warningText(w: SheetWarning): string {
    switch (w.kind) {
      case 'unassigned':
        return `${w.hours} h has no timesheet code yet`;
      case 'nothing_logged':
        return `${weekdayLong(w.date)} has nothing logged`;
      case 'long_day':
        return `${weekdayLong(w.date)} is over 12 h (${w.hours} h)`;
    }
  }

  function setGroup(g: GroupBy) {
    groupBy = g;
    void attempt(() => api.savePref('timesheet.group_by', g)).then((p) => {
      if (p) app.prefs = p;
    });
  }

  const shownDays = $derived(
    (sheet?.days ?? []).filter((d, i) => i < 5 || d.lines.length > 0),
  );
  const current = $derived(groupBy ?? sheet?.groupBy ?? 'code');
</script>

<div class="ts">
  <div class="ts-top">
    <h2>What to enter · week of {sheet ? dayMonth(sheet.weekStart) : ''}</h2>
    <span class="nav">
      <button class="btn" type="button" aria-label="Previous week" onclick={() => (anchor = addDays(anchor, -7))}>
        <Icon name="left" size={15} />
      </button>
      <button class="btn" type="button" aria-label="Next week" disabled={sheet !== null && addDays(sheet.weekStart, 7) > app.today} onclick={() => (anchor = addDays(anchor, 7))}>
        <Icon name="right" size={15} />
      </button>
      {#if sheet && addDays(sheet.weekStart, 7) <= app.today}
        <button class="btn" type="button" onclick={() => (anchor = app.today)}>This week</button>
      {/if}
    </span>
    <button class="btn primary" type="button" disabled={!sheet?.tsv} onclick={() => sheet && copy(sheet.tsv, 'Week copied as columns. Paste it into a spreadsheet or your timesheet.')}>
      <Icon name="copy" size={14} /> Copy whole week
    </button>
    <div class="seg group" role="group" aria-label="Group by">
      <button type="button" aria-pressed={current === 'code'} onclick={() => setGroup('code')}>By timesheet code</button>
      <button type="button" aria-pressed={current === 'project'} onclick={() => setGroup('project')}>By project</button>
    </div>
  </div>

  {#if sheet}
    <div class="paper effort rise">
      <div>
        <span class="lbl">Your effort this week</span>
        <div class="stack" aria-hidden="true">
          {#each sheet.effort as e (e.projectId ?? 'none')}
            <i style:flex-grow={e.minutes} style:background-color={projectColor(e.projectId)}></i>
          {/each}
        </div>
        <div class="leg">
          {#each sheet.effort as e (e.projectId ?? 'none')}
            <span><i class="dot" style:--c={projectColor(e.projectId)}></i>{e.name} {e.hours} h</span>
          {:else}
            <span>Nothing logged this week yet.</span>
          {/each}
        </div>
      </div>
      <div class="kpi"><div class="big mono">{sheet.hours}</div><span class="lbl">hours</span></div>
      <div class="kpi"><div class="big mono">{sheet.ticketCount}</div><span class="lbl">tickets</span></div>
      <div class="kpi"><div class="big mono">{sheet.entryCount}</div><span class="lbl">entries</span></div>
    </div>

    {#if sheet.warnings.length > 0}
      <div class="warns" role="list" aria-label="Check before you submit">
        {#each sheet.warnings as w, i (i)}
          <span class="warn" role="listitem">{warningText(w)}</span>
        {/each}
      </div>
    {/if}

    <div class="days">
      {#each shownDays as d, di (d.date)}
        {@const n = doneCount(d)}
        <article class="paper tday rise" class:shut={shut[d.date] ?? allDone(d)} style:--i={di}>
          <header>
            <button class="head" type="button" aria-expanded={!(shut[d.date] ?? allDone(d))} onclick={() => (shut[d.date] = !(shut[d.date] ?? allDone(d)))}>
              <b>{weekdayLong(d.date)} {dayOfMonth(d.date)}</b>
              <span class="mono hrs-day">{d.hours} h</span>
            </button>
            {#if allDone(d)}<span class="done-stamp">ENTERED</span>{/if}
            <span class="prog">
              <span>{d.lines.length ? `${n} of ${d.lines.length} entered` : 'nothing to enter'}</span>
              <span class="pbar"><i style:width="{d.lines.length ? (n / d.lines.length) * 100 : 0}%"></i></span>
            </span>
          </header>
          <div class="lines">
            <div>
              {#each d.lines as line (line.key)}
                {@const ok = done(d.date, line.key)}
                <div class="line" class:ok class:unas={line.unassigned}>
                  <input
                    type="checkbox"
                    checked={ok}
                    aria-label="Entered {line.label} on {weekdayShort(d.date)}"
                    onchange={(e) => tick(d, line.key, (e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="code">
                    <small>{line.area}</small>
                    <b class:warn-text={line.unassigned} style:color={current === 'project' ? projectColor(line.projectId) : null}>{line.label}</b>
                  </span>
                  <span class="hrs mono">{line.hours}</span>
                  <span class="cm">
                    {#each line.items as item, ii (ii)}
                      {#if ii > 0}<span class="sep"> · </span>{/if}
                      {item.title}
                      {#each item.tickets as t (t)}<span class="chip tk">{t}</span>{/each}
                    {/each}
                  </span>
                  <button class="cpy" type="button" onclick={() => copy(line.comment, 'Comment copied. Paste it into your timesheet.')}>
                    Copy comment
                  </button>
                </div>
              {/each}
            </div>
          </div>
        </article>
      {/each}
    </div>
    <p class="help-note">
      Open your timesheet next to this. Each line is one thing to type: the code, the hours and a
      ready-made comment with your tickets. Tick it off as you go.
    </p>
  {/if}
</div>

<style>
  .ts {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 1000px;
  }

  .ts-top {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  h2 {
    margin: 0 4px 0 0;
    font: 650 24px var(--display);
  }

  .nav {
    display: inline-flex;
    gap: 4px;
  }

  .nav .btn {
    padding: 6px 8px;
  }

  .group {
    margin-left: auto;
  }

  .effort {
    display: grid;
    grid-template-columns: 1.6fr repeat(3, minmax(0, 1fr));
    gap: 12px;
    padding: 14px 16px;
    align-items: center;
  }

  .stack {
    display: flex;
    height: 14px;
    border-radius: 7px;
    overflow: hidden;
    margin-top: 8px;
    background: color-mix(in srgb, var(--ink) 6%, transparent);
  }

  .stack i {
    display: block;
    transition:
      flex-grow 0.6s var(--spring),
      background-color 0.4s;
  }

  .leg {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--ink-2);
  }

  .leg span {
    display: inline-flex;
    gap: 5px;
    align-items: center;
  }

  .big {
    font-weight: 700;
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .warns {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .warn {
    font-size: 12.5px;
    padding: 5px 10px;
    border-radius: 99px;
    background: var(--warn-wash);
    color: var(--warn);
    font-weight: 600;
  }

  .days {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tday {
    padding: 0;
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    flex-wrap: wrap;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    border: 0;
    background: none;
    padding: 2px 0;
    cursor: pointer;
    color: inherit;
  }

  .head b {
    font: 650 17px var(--display);
  }

  .hrs-day {
    font-weight: 700;
  }

  .prog {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--mut);
  }

  .pbar {
    width: 90px;
    height: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    overflow: hidden;
  }

  .pbar i {
    display: block;
    height: 100%;
    background: var(--ok);
    transition: width calc(0.5s * var(--m)) var(--spring);
  }

  .done-stamp {
    font: 700 11px var(--mono);
    letter-spacing: 0.12em;
    color: var(--ok);
    border: 2px solid var(--ok);
    border-radius: 5px;
    padding: 1px 7px;
    transform: rotate(-4deg);
    animation: stamp calc(0.45s * var(--m)) var(--bounce) both;
  }

  @keyframes stamp {
    from {
      transform: rotate(-4deg) scale(2);
      opacity: 0;
    }
  }

  .lines {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows calc(0.45s * var(--m)) var(--ease);
  }

  .shut .lines {
    grid-template-rows: 0fr;
  }

  .lines > div {
    overflow: hidden;
  }

  .line {
    display: grid;
    grid-template-columns: 26px minmax(150px, 1.1fr) 58px minmax(0, 2fr) auto;
    gap: 12px;
    align-items: center;
    padding: 10px 16px;
    border-top: 1px solid var(--rule);
    transition:
      opacity 0.3s,
      background 0.3s;
  }

  .line.unas {
    background: var(--warn-wash);
  }

  .line input[type='checkbox'] {
    width: 18px;
    height: 18px;
    accent-color: var(--ok);
    cursor: pointer;
  }

  .code small {
    display: block;
    color: var(--mut);
    font-size: 11.5px;
  }

  .code b {
    font-weight: 600;
  }

  .warn-text {
    color: var(--warn);
  }

  .hrs {
    font-weight: 700;
    font-size: 15px;
    text-align: right;
  }

  .cm {
    font-size: 13px;
    color: var(--ink-2);
    min-width: 0;
  }

  .tk {
    margin: 1px 2px;
    cursor: default;
  }

  .line.ok {
    opacity: 0.5;
  }

  .line.ok .hrs,
  .line.ok .code b {
    text-decoration: line-through;
    text-decoration-thickness: 2px;
  }

  .cpy {
    border: 1px solid var(--rule);
    background: var(--paper);
    border-radius: 6px;
    padding: 4px 9px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink);
  }

  .cpy:hover {
    background: var(--ink);
    color: var(--paper);
  }

  .help-note {
    color: var(--mut);
    font-size: 13px;
    margin: 0;
  }

  @media (max-width: 760px) {
    .effort {
      grid-template-columns: 1fr 1fr;
    }
    .line {
      grid-template-columns: 26px 1fr 50px;
    }
    .cm,
    .cpy {
      grid-column: 2 / -1;
    }
  }
</style>
