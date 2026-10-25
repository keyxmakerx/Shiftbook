<script lang="ts">
  // The Desk: today's card (date, hours ring, week total, "Where was I?", Log work, entries) beside the
  // Done tickets tray and the Inbox.
  import { fmtHours, fromIso, longDay, relativeDay, weekdayLong } from '../lib/format';
  import { countTo } from '../lib/motion';
  import { app, ribbonDay, ribbonWeekOf } from '../lib/store.svelte';
  import { hasHome } from '../lib/words';
  import EntryRow from '../components/EntryRow.svelte';
  import InboxCard from '../components/InboxCard.svelte';
  import LogWork from '../components/LogWork.svelte';
  import DeskNotes from '../components/DeskNotes.svelte';
  import TrayCard from '../components/TrayCard.svelte';
  import WhereWasI from '../components/WhereWasI.svelte';
  import WrapUp from '../components/WrapUp.svelte';

  const RING = 119.4;

  // The day's target from Settings → Your day (Monday first); 8 h until it's set.
  const target = $derived(app.prefs?.dayTargets[(fromIso(app.selected).getDay() + 6) % 7] ?? 480);

  const dayMinutes = $derived(ribbonDay(app.selected)?.minutes ?? 0);
  const weekMinutes = $derived(ribbonWeekOf(app.selected)?.minutes ?? 0);

  // Home (round 15): with no hours to aim for, count what got done instead of a ring.
  const countDone = $derived(hasHome(app.prefs?.useMode) && !target);
  const doneCount = $derived(app.entries.filter((e) => !e.isTray).length);

  let shownDay = $state(0);
  let shownWeek = $state(0);
  let dayFrom = 0;
  let weekFrom = 0;

  $effect(() => {
    const to = dayMinutes;
    return countTo(dayFrom, to, (v) => {
      shownDay = v;
      dayFrom = v;
    });
  });

  $effect(() => {
    const to = weekMinutes;
    return countTo(weekFrom, to, (v) => {
      shownWeek = v;
      weekFrom = v;
    });
  });
</script>

<div class="desk">
  <article class="paper todaycard rise">
    <div class="sum">
      <div class="when">
        <span class="lbl">{relativeDay(app.selected, app.today)}</span>
        <h1>{longDay(app.selected)}</h1>
      </div>
      {#if countDone}
        <div class="count" aria-live="polite">
          <div class="big">{doneCount}</div>
          <span class="lbl">{doneCount === 1 ? 'thing' : 'things'} done {app.selected === app.today ? 'today' : 'this day'}</span>
        </div>
      {:else}
        <div class="ring" title={target ? `${fmtHours(dayMinutes)} of ${fmtHours(target)} hours` : 'No target on this day'}>
          <svg viewBox="0 0 46 46" aria-hidden="true">
            <circle class="track" cx="23" cy="23" r="19" />
            <circle
              class="fill"
              cx="23"
              cy="23"
              r="19"
              stroke-dasharray={RING}
              stroke-dashoffset={RING * (1 - (target ? Math.min(shownDay / target, 1) : 0))}
            />
          </svg>
          <div><div class="big">{fmtHours(shownDay)}</div><span class="lbl">{target ? `of ${fmtHours(target)} h` : `no target ${weekdayLong(app.selected)}s`}</span></div>
        </div>
        <div><div class="big week">{fmtHours(shownWeek)}</div><span class="lbl">this week</span></div>
      {/if}
    </div>

    <WhereWasI />

    <LogWork />

    <div class="list" aria-label="Entries">
      {#each app.entries as entry, i (entry.id)}
        <EntryRow {entry} index={i} />
      {:else}
        <p class="empty-note">
          {#if hasHome(app.prefs?.useMode)}
            Nothing down for this day yet. Type what you did in the box above and press Done, or press Ctrl+Space anywhere.
          {:else}
            Nothing logged for this day yet. Type in the box above, paste finished tickets into the tray,
            or press Ctrl+Space anywhere.
          {/if}
        </p>
      {/each}
    </div>
  </article>

  <div class="side">
    <WrapUp />
    <div class="rise" style:--i={1}><TrayCard /></div>
    <div class="rise" style:--i={2}><DeskNotes /></div>
    <div class="rise" style:--i={3}><InboxCard /></div>
  </div>
</div>

<style>
  .desk {
    display: grid;
    grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }

  .todaycard {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .sum {
    display: flex;
    gap: 22px;
    align-items: center;
    flex-wrap: wrap;
  }

  .when {
    flex: 1 1 260px;
  }

  h1 {
    margin: 0;
    font: 650 34px/1.05 var(--display);
    letter-spacing: -0.02em;
  }

  .big {
    font: 700 30px var(--mono);
    letter-spacing: -0.02em;
  }

  .big.week {
    font-size: 20px;
  }

  .count {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .ring {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .ring svg {
    width: 46px;
    height: 46px;
    transform: rotate(-90deg);
  }

  .ring circle {
    fill: none;
    stroke-width: 6;
  }

  .track {
    stroke: color-mix(in srgb, var(--ink) 10%, transparent);
  }

  .fill {
    stroke: var(--ok);
    stroke-linecap: round;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  @media (max-width: 760px) {
    .desk {
      grid-template-columns: 1fr;
    }
  }
</style>
