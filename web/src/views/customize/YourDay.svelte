<script lang="ts">
  // Settings → Your day: the Day ring's target for each weekday, the optional
  // end-of-day wrap-up (round 7), the optional timer (off by default) and the headings of
  // "Copy for stand-up" and your own time buttons (round 15).
  import { api } from '../../lib/api';
  import { MAX_TIME_BUTTONS } from '../../lib/api/generated/constants';
  import { fmtMinutes } from '../../lib/format';
  import { app, DEFAULT_TIMES, savePref, timeButtons } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';

  /** Most time buttons (core's `MAX_TIME_BUTTONS`). */
  const MAX_TIMES = MAX_TIME_BUTTONS;

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

  async function setWords(done: string, next: string) {
    if (!done.trim() || !next.trim()) return;
    await savePref('day.standup_words', `${done.trim()}|${next.trim()}`);
  }

  const times = $derived(timeButtons());
  const ownTimes = $derived(times.join() !== DEFAULT_TIMES.join());
  let newTime = $state('');

  async function saveTimes(list: number[]) {
    await savePref('day.time_buttons', list.length ? list.join(',') : '');
  }

  async function addTime() {
    const typed = newTime.trim();
    if (!typed) return;
    // "20" means minutes; "20m", "1.5h" and "1h 30m" read the same way Log work does.
    const asTime = /^\d+$/.test(typed) ? `${typed}m` : typed;
    const read = await api.parseCapture(asTime, app.today).catch(() => null);
    const minutes = read?.minutes ?? null;
    if (minutes === null || minutes < 1 || minutes > 720) {
      toast('Type a time like 20m or 1.5h (up to 12 hours).');
      return;
    }
    newTime = '';
    if (!times.includes(minutes)) await saveTimes([...times, minutes]);
  }

  let wrapTime = $state('17:00');
  $effect(() => {
    if (prefs?.wrapupAt) wrapTime = prefs.wrapupAt;
  });
</script>

{#if prefs}
  <article class="paper cz-card">
    <h3>Daily target</h3>
    <p class="sub">How many hours fill the ring on Day. Put 0 for days you don't work.</p>
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
    <h3>Time buttons</h3>
    <p class="sub">Up to five, in Log work, on jots and for tickets. Pick the times you really use.</p>
    <div class="cz-row times">
      {#each times as m (m)}
        <button
          type="button"
          class="btn mono"
          title="Take {fmtMinutes(m)} off"
          aria-label="Take {fmtMinutes(m)} off"
          disabled={times.length === 1}
          onclick={() => saveTimes(times.filter((t) => t !== m))}
        >{fmtMinutes(m)} ✕</button>
      {/each}
      {#if times.length < MAX_TIMES}
        <form class="add" onsubmit={(e) => { e.preventDefault(); void addTime(); }}>
          <input class="text-in mono" bind:value={newTime} placeholder="20m" aria-label="New time button, like 20m or 1.5h" />
          <button type="submit" class="btn">+ Add</button>
        </form>
      {:else}
        <span class="note">Five is the most. Take one off to add another.</span>
      {/if}
      {#if ownTimes}
        <button type="button" class="btn" onclick={() => saveTimes([])}>
          Back to {DEFAULT_TIMES.map(fmtMinutes).join(' ')}
        </button>
      {/if}
    </div>
    <p class="sub preview">In Log work: {#each times as m (m)}<span class="prev-btn mono">{fmtMinutes(m)}</span>{/each}</p>
  </article>

  <article class="paper cz-card">
    <h3>End-of-day wrap-up</h3>
    <p class="sub">
      At this time Day and the Desk show a short wrap-up: what you logged, anything missing a timesheet
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

  <article class="paper cz-card">
    <h3>Copy for stand-up</h3>
    <p class="sub">
      "Copy for stand-up" on Where was I? (or in Find) copies what you did on your last day with
      work, then what you start with today. Name the two parts the way your team says them.
    </p>
    <div class="cz-row">
      <label class="word">
        <span>What you did</span>
        <input
          class="text-in"
          maxlength="20"
          value={prefs.standupWords[0]}
          onchange={(e) => setWords((e.currentTarget as HTMLInputElement).value, prefs.standupWords[1])}
        />
      </label>
      <label class="word">
        <span>What's next</span>
        <input
          class="text-in"
          maxlength="20"
          value={prefs.standupWords[1]}
          onchange={(e) => setWords(prefs.standupWords[0], (e.currentTarget as HTMLInputElement).value)}
        />
      </label>
    </div>
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

  .times {
    flex-wrap: wrap;
    gap: 8px;
  }

  .add {
    display: inline-flex;
    gap: 6px;
  }

  .add input {
    width: 80px;
  }

  .preview {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
  }

  .prev-btn {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px 8px;
    color: var(--ink);
  }

  .word {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: var(--ink-2);
  }

  .word input {
    width: 160px;
  }
</style>
