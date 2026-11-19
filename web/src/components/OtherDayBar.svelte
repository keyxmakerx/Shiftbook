<script lang="ts">
  // A strip under the ribbon whenever the Desk or Day shows a day that isn't today (round 21,
  // "Nothing should move or vanish on its own"): it says which day, with Back to today.
  import { dayDistance, longDay } from '../lib/format';
  import { app, selectDay } from '../lib/store.svelte';

  const shown = $derived(app.ready && app.selected !== app.today && (app.view === 'desk' || app.view === 'day'));
</script>

{#if shown}
  <div class="bar" role="status">
    <span class="what">Showing <b>{longDay(app.selected)}</b> <span class="ago">· {dayDistance(app.selected, app.today).toLowerCase()}</span></span>
    <span class="grow"></span>
    <button class="btn primary" type="button" onclick={() => void selectDay(app.today)}>Back to today</button>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 6px 14px;
    border-bottom: 1px solid var(--rule);
    background: var(--warn-wash);
    font-size: 13.5px;
  }

  .ago {
    color: var(--mut);
  }

  .grow {
    flex: 1;
  }
</style>
