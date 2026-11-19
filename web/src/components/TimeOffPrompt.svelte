<script lang="ts">
  // "Book time off…" (round 21): pick the first and last day off and see what gets booked
  // before booking it. Which days count comes from the app's core; this only shows them.
  import { api } from '../lib/api';
  import type { TimeOffPlan } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { dayOfMonth, fmtMinutes, longDay, weekdayShort } from '../lib/format';
  import { hasTimesheet } from '../lib/words';
  import { app } from '../lib/store.svelte';
  import { offButton, offSkipped, offSummary } from '../lib/timeOff';
  import { bookTimeOff, closeTimeOff, timeOffPrompt } from '../lib/timeOffPrompt.svelte';

  const ISO = /^\d{4}-\d{2}-\d{2}$/;

  let from = $state('');
  let to = $state('');
  let plan = $state<TimeOffPlan | null>(null);
  let problem = $state('');
  let busy = $state(false);

  $effect(() => {
    void timeOffPrompt.serial;
    from = timeOffPrompt.from;
    to = timeOffPrompt.from;
  });

  // Ask the app what these days come to, each time they change.
  $effect(() => {
    if (!timeOffPrompt.open) return;
    const [a, b] = [from, to];
    if (!ISO.test(a) || !ISO.test(b)) {
      plan = null;
      problem = 'Pick the first and the last day off.';
      return;
    }
    let stale = false;
    api.timeOffPlan(a, b).then(
      (p) => {
        if (stale) return;
        plan = p;
        problem = '';
      },
      () => {
        if (stale) return;
        plan = null;
        problem = b < a ? 'The last day is before the first.' : 'That\'s more than two months. Book it in two parts.';
      },
    );
    return () => {
      stale = true;
    };
  });

  const code = $derived.by(() => {
    if (!hasTimesheet(app.prefs)) return '';
    const c = app.codes.find((k) => k.id === app.prefs?.timeOffCode);
    return c ? c.code : '';
  });
  const ready = $derived(!!plan && plan.booked > 0 && !busy);

  function setFrom(value: string) {
    from = value;
    // The last day follows when the first moves past it.
    if (ISO.test(value) && (!ISO.test(to) || to < value)) to = value;
  }

  async function ok() {
    if (!ready || !plan) return;
    busy = true;
    const dates = plan.days.filter((d) => d.skip === null).map((d) => d.date);
    const done = await bookTimeOff(from, to, dates);
    busy = false;
    if (done) closeTimeOff();
  }

  function keys(e: KeyboardEvent) {
    if (e.key === 'Enter') void ok();
    else if (e.key === 'Escape') {
      e.stopPropagation();
      closeTimeOff();
    }
  }
</script>

{#if timeOffPrompt.open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={closeTimeOff}></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="box paper" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="off-title" onkeydown={keys}>
    <h2 id="off-title">Book time off</h2>
    <div class="range">
      <label>
        <span class="lbl">First day off</span>
        <input class="text-in" type="date" value={from} oninput={(e) => setFrom(e.currentTarget.value)} use:focusNow />
      </label>
      <label>
        <span class="lbl">Last day off</span>
        <input class="text-in" type="date" min={from} bind:value={to} />
      </label>
    </div>
    {#if plan}
      <ul class="days" aria-label="The days in between">
        {#each plan.days as d (d.date)}
          <li
            class:skip={d.skip !== null}
            title="{longDay(d.date)}: {d.skip === 'free' ? 'not a work day, skipped' : d.skip === 'logged' ? 'already has something logged, skipped' : fmtMinutes(d.minutes)}"
          >
            <span>{weekdayShort(d.date)}</span>
            <b>{dayOfMonth(d.date)}</b>
          </li>
        {/each}
      </ul>
      <p class="sum" aria-live="polite">
        {#if plan.booked > 0}
          <strong>{offSummary(plan)}</strong>{code ? ` on ${code}` : ''}, {plan.booked === 1 ? 'as' : 'each as'} a “Holiday” entry.
        {/if}
        {offSkipped(plan)}
      </p>
      {#if plan.booked > 0}
        <p class="hint">The ribbon shows days off in grey, and the bell stays quiet on them.</p>
      {/if}
    {:else}
      <p class="sum" aria-live="polite">{problem}</p>
    {/if}
    <div class="row">
      <button type="button" class="btn" onclick={closeTimeOff}>Cancel</button>
      <button type="button" class="btn primary" disabled={!ready} onclick={() => void ok()}>{offButton(plan)}</button>
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: color-mix(in srgb, var(--ink) 18%, transparent);
  }

  .box {
    position: fixed;
    z-index: 81;
    left: 50%;
    top: 20%;
    transform: translateX(-50%);
    width: min(480px, calc(100% - 32px));
    max-height: 70%;
    overflow: auto;
    padding: 16px 18px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font: 700 17px var(--display);
  }

  .range {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .range label {
    flex: 1 1 180px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .days {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .days li {
    min-width: 44px;
    padding: 4px 6px;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
    font-size: 11.5px;
  }

  .days li b {
    font: 700 14px var(--mono);
  }

  .days li.skip {
    background: none;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--ink) 14%, transparent);
    color: var(--mut);
  }

  .days li.skip b {
    font-weight: 500;
    text-decoration: line-through;
  }

  .sum {
    margin: 0;
    font-size: 13.5px;
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
