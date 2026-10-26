<script lang="ts">
  // First run, after the recovery key (round 15): what's Shiftbook for? Work keeps it as it
  // was; Home hides the Timesheet and the hours ring and uses everyday words; Both has work and
  // home on one Desk. It only changes words, defaults and what shows, so it can be switched any
  // time in Settings → Look.
  import UseModeChoice from '../components/UseModeChoice.svelte';
  import type { UseMode } from '../lib/api';
  import { api } from '../lib/api';
  import { errorMessage } from '../lib/messages';

  let { onDone }: { onDone: () => void } = $props();
  let mode = $state<UseMode>('work');
  let working = $state(false);
  let error = $state('');

  async function carryOn(e: SubmitEvent) {
    e.preventDefault();
    working = true;
    error = '';
    try {
      await api.setUseMode(mode);
      onDone();
    } catch (err) {
      error = errorMessage(err);
      working = false;
    }
  }
</script>

<main class="narrow wide">
  <h1>What's Shiftbook for?</h1>
  <p class="lead">This only changes words and what shows. You can switch any time in Settings → Look.</p>
  <form onsubmit={carryOn} class="stack">
    <UseModeChoice bind:mode />
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <button type="submit" class="primary" disabled={working}>Carry on</button>
  </form>
</main>

<style>
  .wide {
    max-width: 760px;
  }
</style>
