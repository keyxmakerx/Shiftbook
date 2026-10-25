<script lang="ts">
  // The end-of-day wrap-up (Settings → Your day, off until a time is set): today's hours,
  // what still needs a timesheet code, the tray, and a short list of what to start with
  // tomorrow. "Where was I?" shows that list the next morning.
  import { tick } from 'svelte';
  import { api } from '../lib/api';
  import { fmtHours, weekdayLong } from '../lib/format';
  import { app, attempt, ribbonDay, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { clockLabel, insertLine, isWrapupTime, MAX_LINES, removeLine } from '../lib/wrapup';

  let now = $state(new Date());
  $effect(() => {
    const t = setInterval(() => (now = new Date()), 30_000);
    return () => clearInterval(t);
  });

  const at = $derived(app.prefs?.wrapupAt ?? null);
  const show = $derived(
    app.selected === app.today &&
      app.wrapupClosed !== app.today &&
      (app.wrapupNow === app.today || (!!at && isWrapupTime(now, at))),
  );

  let lines = $state(['']);
  let uncoded = $state(0);
  let saving = $state(false);
  let listEl: HTMLElement | undefined = $state();

  // Load what's already on the list and what needs a code each time the card appears.
  $effect(() => {
    if (!show) return;
    const today = app.today;
    void attempt(async () => {
      const [items, sheet] = await Promise.all([api.nextUp(), api.weekSheet(today, today, null)]);
      const open = items.filter((i) => !i.done).map((i) => i.text);
      lines = open.length ? open : [''];
      const day = sheet.days.find((d) => d.date === today);
      uncoded = (day?.lines ?? [])
        .filter((l) => l.unassigned)
        .reduce((n, l) => n + l.items.reduce((m, it) => m + it.entryIds.length, 0), 0);
    });
  });

  const dayMinutes = $derived(ribbonDay(app.today)?.minutes ?? 0);
  const trayCount = $derived(app.tray?.date === app.today ? app.tray.items.length : 0);

  async function focusLine(i: number) {
    await tick();
    listEl?.querySelectorAll('input')[i]?.focus();
  }

  function onKey(e: KeyboardEvent, i: number) {
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (lines.length < MAX_LINES) {
        lines = insertLine(lines, i);
        void focusLine(i + 1);
      }
    } else if (e.key === 'Backspace' && input.value === '' && lines.length > 1) {
      e.preventDefault();
      lines = removeLine(lines, i);
      void focusLine(Math.max(0, i - 1));
    } else if (e.key === 'ArrowDown' && i < lines.length - 1) {
      e.preventDefault();
      void focusLine(i + 1);
    } else if (e.key === 'ArrowUp' && i > 0) {
      e.preventDefault();
      void focusLine(i - 1);
    }
  }

  async function done() {
    saving = true;
    const saved = await attempt(() => api.setNextUp(lines));
    saving = false;
    if (!saved) return;
    app.wrapupClosed = app.today;
    toast(
      saved.length
        ? `Saved ${saved.length === 1 ? 'one thing' : `${saved.length} things`} for tomorrow. Good work today.`
        : 'Done for the day. Good work today.',
    );
  }

  function later() {
    app.wrapupClosed = app.today;
    toast('Wrap-up put away until tomorrow', 'Undo', () => (app.wrapupClosed = ''));
  }
</script>

{#if show && at}
  <article class="paper wrap rise" aria-labelledby="wrap-h">
    <div class="head">
      <h3 id="wrap-h">Wrapping up {weekdayLong(app.today)}</h3>
      <span class="when">{clockLabel(at)} · you chose this time</span>
    </div>

    <div class="stats">
      <div><b>{fmtHours(dayMinutes)} h</b><span class="lbl">logged today</span></div>
      <button
        type="button"
        class="stat"
        class:warn={uncoded > 0}
        title="Open the Timesheet to pick codes"
        onclick={() => setView('timesheet')}
      >
        <b>{uncoded}</b><span class="lbl">{uncoded === 1 ? 'needs a code' : 'need a code'}</span>
      </button>
      <div><b>{trayCount}</b><span class="lbl">{trayCount === 1 ? 'ticket in tray' : 'tickets in tray'}</span></div>
    </div>

    <div class="tomorrow">
      <span class="lbl" id="wrap-list">Tomorrow I start with…</span>
      <ol bind:this={listEl} aria-labelledby="wrap-list">
        {#each lines as _, i (i)}
          <li class:first={i === 0}>
            {#if i === 0}<span class="tag">First</span>{:else}<span class="box" aria-hidden="true"></span>{/if}
            <input
              class="text-in"
              bind:value={lines[i]}
              maxlength="200"
              aria-label={i === 0 ? 'First thing tomorrow' : `Then, item ${i + 1}`}
              placeholder={i === 0 ? 'e.g. finish the printer ticket' : 'and then…'}
              onkeydown={(e) => onKey(e, i)}
            />
          </li>
        {/each}
      </ol>
      <p class="hint">
        {lines.length < MAX_LINES ? 'Enter adds another line.' : `That's the most (${MAX_LINES}).`}
        "Where was I?" shows the first one tomorrow, with the rest as a checklist.
      </p>
    </div>

    <div class="acts">
      <button class="btn" type="button" onclick={later}>Not today</button>
      <button class="btn primary" type="button" disabled={saving} onclick={done}>Done for the day</button>
    </div>
  </article>
{/if}

<style>
  .wrap {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 3px solid var(--accent);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    font: 650 18px var(--display);
  }

  .when,
  .hint {
    color: var(--mut);
    font-size: 12px;
  }

  .hint {
    margin: 2px 0 0;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .stats b {
    display: block;
    font: 700 22px var(--mono);
  }

  .stat {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: inherit;
    cursor: pointer;
    border-radius: 6px;
  }

  .stat.warn b {
    color: var(--warn);
  }

  .tomorrow ol {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  li input {
    flex: 1;
    min-width: 0;
  }

  .tag {
    font: 600 11px var(--ui);
    padding: 2px 7px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--ink);
    min-width: 40px;
    text-align: center;
  }

  .box {
    width: 13px;
    height: 13px;
    margin: 0 13px 0 14px;
    border: 1.5px solid var(--mut);
    border-radius: 3px;
    flex: none;
  }

  .acts {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
</style>
