<script lang="ts">
  // The optional timer's strip under the ribbon: the clock while it runs, a paused timer
  // after unlock, and after Stop what to add. Nothing is added without a click; no alarms.
  import { fmtMinutes } from '../lib/format';
  import { clockText } from '../lib/timerClock';
  import { resumeTimer, settleTimer, stopTimer, timer } from '../lib/timer.svelte';
</script>

{#if timer.target}
  <div class="bar" class:asking={!!timer.offer} role="status" aria-live="polite">
    {#if timer.offer}
      <span class="what">Add to <b>{timer.target.title}</b>:</span>
      {#if timer.offer.minutes > 0}
        <button class="btn primary" type="button" onclick={() => settleTimer(timer.offer?.minutes ?? 0)}>
          Add {fmtMinutes(timer.offer.minutes)}
        </button>
      {/if}
      {#if timer.offer.withoutAway !== null}
        <button class="btn" type="button" onclick={() => settleTimer(timer.offer?.withoutAway ?? 0)}>
          {timer.offer.withoutAway > 0 ? `Add ${fmtMinutes(timer.offer.withoutAway)}` : 'Add nothing'}
          <small>leaves out {fmtMinutes(timer.offer.awayMinutes)} away</small>
        </button>
      {/if}
      {#if timer.offer.minutes === 0}<span class="note">Less than a minute, so nothing to add.</span>{/if}
      <span class="grow"></span>
      {#if !timer.next}<button class="btn" type="button" onclick={resumeTimer}>Keep counting</button>{/if}
      <button class="btn" type="button" onclick={() => settleTimer(0)}>Don't add</button>
    {:else}
      <span class="dot" class:live={timer.running} aria-hidden="true"></span>
      <span class="what">{timer.running ? 'Now doing' : 'Paused'}: <b>{timer.target.title}</b></span>
      <span class="clock">{clockText(timer.shown)}</span>
      <span class="grow"></span>
      {#if !timer.running}<button class="btn" type="button" onclick={resumeTimer}>▶ Go on</button>{/if}
      <button class="btn" type="button" onclick={stopTimer}>■ Stop</button>
    {/if}
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
    background: color-mix(in srgb, var(--accent) 7%, var(--paper));
    font-size: 13.5px;
  }

  .bar.asking {
    background: color-mix(in srgb, var(--accent) 13%, var(--paper));
  }

  .what {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50ch;
  }

  .clock {
    font: 700 15px var(--mono);
    font-variant-numeric: tabular-nums;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--mut);
  }

  .dot.live {
    background: var(--ok);
  }

  .grow {
    flex: 1;
  }

  small,
  .note {
    color: var(--mut);
    font-size: 12px;
  }

  small {
    margin-left: 4px;
  }
</style>
