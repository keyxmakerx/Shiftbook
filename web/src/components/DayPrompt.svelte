<script lang="ts">
  // "Pick a day…" (round 15): a date box over everything, for "Bring up…".
  import { answerDay, dayPrompt } from '../lib/dayPrompt.svelte';
  import { focusNow } from '../lib/focus';
  import { longDay } from '../lib/format';

  let day = $state('');

  $effect(() => {
    void dayPrompt.serial;
    day = dayPrompt.min;
  });

  const valid = $derived(/^\d{4}-\d{2}-\d{2}$/.test(day) && day >= dayPrompt.min);

  function ok() {
    if (valid) answerDay(day);
  }
</script>

{#if dayPrompt.open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={() => answerDay(null)}></div>
  <div class="box paper" role="dialog" aria-modal="true" aria-labelledby="day-prompt-title">
    <h2 id="day-prompt-title">{dayPrompt.title}</h2>
    <input
      class="text-in"
      type="date"
      min={dayPrompt.min}
      bind:value={day}
      use:focusNow
      aria-label="Day"
      onkeydown={(e) => {
        if (e.key === 'Enter') ok();
        else if (e.key === 'Escape') {
          e.stopPropagation();
          answerDay(null);
        }
      }}
    />
    <p class="hint">{valid ? longDay(day) : 'Pick a day after today.'} A day off moves to your next work day.</p>
    <div class="row">
      <button type="button" class="btn" onclick={() => answerDay(null)}>Cancel</button>
      <button type="button" class="btn primary" disabled={!valid} onclick={ok}>Bring it up then</button>
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
    top: 30%;
    transform: translateX(-50%);
    width: min(340px, calc(100% - 32px));
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
