<script lang="ts">
  // First run, after "What's Shiftbook for?" (round 21, "A friendlier first day"): one screen
  // of helpers to turn on. Each is an ordinary setting (Settings → Your day), so Skip changes
  // nothing and everything here can be changed later.
  import type { UseMode } from '../lib/api';
  import { api } from '../lib/api';
  import { daysSummary, helperDefaults, helperPrefs, homeOnly } from '../lib/helpers';
  import { errorMessage } from '../lib/messages';

  let { mode, onDone }: { mode: UseMode; onDone: () => void } = $props();
  let choice = $state(helperDefaults());
  let working = $state(false);
  let error = $state('');

  const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const home = $derived(homeOnly(mode));

  async function start(e: SubmitEvent) {
    e.preventDefault();
    working = true;
    error = '';
    try {
      for (const [key, value] of helperPrefs(choice, mode)) await api.savePref(key, value);
      onDone();
    } catch (err) {
      error = errorMessage(err);
      working = false;
    }
  }
</script>

<main class="narrow wide">
  <h1>A few helpers</h1>
  <p class="lead">Turn on what sounds useful. You can change any of these later in Settings → Your day.</p>
  <form onsubmit={start} class="stack">
    <div class="helper" class:on={choice.wrapup}>
      <label class="tick">
        <input type="checkbox" bind:checked={choice.wrapup} />
        <span><b>End-of-day wrap-up</b><small>A short card at the end of your day: what you logged, and what you'll start with tomorrow.</small></span>
      </label>
      {#if choice.wrapup}
        <label class="extra">
          <span>At</span>
          <input type="time" class="text-in" bind:value={choice.wrapupAt} aria-label="Wrap-up time" />
        </label>
      {/if}
    </div>

    <div class="helper" class:on={choice.timer}>
      <label class="tick">
        <input type="checkbox" bind:checked={choice.timer} />
        <span><b>Timer</b><small>A ▶ Start button on entries, if you'd rather time things than guess. Nothing is added unless you choose it.</small></span>
      </label>
    </div>

    {#if !home}
      <div class="helper" class:on={choice.timesheet}>
        <label class="tick">
          <input type="checkbox" bind:checked={choice.timesheet} />
          <span><b>I fill in a timesheet</b><small>Shows the Timesheet, its codes and the “needs a code” reminders. Leave it off if you just want a log.</small></span>
        </label>
      </div>

      <div class="helper on">
        <div class="tick plain">
          <span><b>Days I work</b><small>For the daily ring and the “nothing logged” dots. 8 hours a day to start with.</small></span>
          <span class="sum">{daysSummary(choice.days)}</span>
        </div>
        <div class="days" role="group" aria-label="Days I work">
          {#each DAYS as d, i (d)}
            <button type="button" class="day" aria-pressed={choice.days[i]} onclick={() => (choice.days[i] = !choice.days[i])}>{d}</button>
          {/each}
        </div>
      </div>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="acts">
      <button type="button" class="btn" disabled={working} onclick={onDone}>Skip</button>
      <button type="submit" class="primary" disabled={working}>Start using Shiftbook</button>
    </div>
  </form>
</main>

<style>
  .wide {
    max-width: 640px;
  }

  .helper {
    border: 1.5px solid var(--rule);
    border-radius: 10px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--paper);
  }

  .helper.on {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--rule));
  }

  .tick {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    cursor: pointer;
  }

  .tick.plain {
    cursor: default;
    justify-content: space-between;
  }

  .tick input {
    width: 20px;
    height: 20px;
    margin-top: 2px;
    flex: none;
  }

  .tick span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tick small {
    color: var(--mut);
    font-size: 13px;
  }

  .sum {
    font: 600 13px var(--mono);
    color: var(--ink-2);
    white-space: nowrap;
  }

  .extra {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 32px;
  }

  .extra input {
    width: auto;
  }

  .days {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .day {
    min-width: 48px;
    min-height: 36px;
    border: 1.5px solid var(--rule);
    border-radius: 99px;
    background: none;
    color: var(--ink);
    font: inherit;
    cursor: pointer;
  }

  .day[aria-pressed='true'] {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
