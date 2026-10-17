<script lang="ts">
  // Settings → Your day: the Desk ring's target for each weekday, the optional
  // end-of-day wrap-up (round 7) and the optional timer (off by default).
  import { app, savePref } from '../../lib/store.svelte';

  const DAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

  const prefs = $derived(app.prefs);

  function hoursOf(minutes: number): string {
    return String(Math.round((minutes / 60) * 100) / 100);
  }

  async function setTarget(day: number, text: string) {
    if (!prefs) return;
    const hours = Number(text.replace(',', '.'));
    if (!Number.isFinite(hours) || hours < 0 || hours > 24) return;
    const targets = [...prefs.dayTargets];
    targets[day] = Math.round(hours * 60);
    await savePref('day.targets', targets.join(','));
  }

  let wrapTime = $state('17:00');
  $effect(() => {
    if (prefs?.wrapupAt) wrapTime = prefs.wrapupAt;
  });
</script>

{#if prefs}
  <article class="paper cz-card">
    <h3>Daily target</h3>
    <p class="sub">How many hours fill the ring on the Desk. Put 0 for days you don't work.</p>
    <div class="days">
      {#each DAYS as name, i (name)}
        <label class="day">
          <span>{name}</span>
          <span class="in">
            <input
              class="text-in mono"
              type="number"
              min="0"
              max="24"
              step="0.25"
              value={hoursOf(prefs.dayTargets[i] ?? 0)}
              onchange={(e) => setTarget(i, (e.currentTarget as HTMLInputElement).value)}
            />
            <small>h</small>
          </span>
        </label>
      {/each}
    </div>
  </article>

  <article class="paper cz-card">
    <h3>End-of-day wrap-up</h3>
    <p class="sub">
      At this time the Desk shows a short wrap-up: what you logged, anything missing a timesheet
      code, and a few lines for what's next tomorrow. Off unless you turn it on.
    </p>
    <div class="cz-row">
      <label class="check-row">
        <input
          type="checkbox"
          checked={!!prefs.wrapupAt}
          onchange={(e) => savePref('day.wrapup_at', (e.currentTarget as HTMLInputElement).checked ? wrapTime : '')}
        />
        Show the wrap-up at
      </label>
      <input
        class="text-in mono time"
        type="time"
        bind:value={wrapTime}
        aria-label="Wrap-up time"
        onchange={() => prefs.wrapupAt && savePref('day.wrapup_at', wrapTime)}
      />
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Timer</h3>
    <p class="sub">
      Adds a ▶ Start button to today's entries. A small clock runs under the top bar, and Stop
      offers to add the time, rounded to 15 minutes. Nothing is added unless you choose it. Off
      unless you turn it on.
    </p>
    <label class="check-row">
      <input
        type="checkbox"
        checked={prefs.timer}
        onchange={(e) => savePref('day.timer', (e.currentTarget as HTMLInputElement).checked ? 'true' : 'false')}
      />
      Show the timer
    </label>
  </article>
{/if}

<style>
  .days {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px 14px;
  }

  .day {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 13.5px;
  }

  .in {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .in input {
    width: 72px;
    text-align: right;
  }

  .in small {
    color: var(--mut);
  }

  .time {
    width: 120px;
  }
</style>
