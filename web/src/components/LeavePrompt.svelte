<script lang="ts">
  // "Wrap up before you go?" (round 21): shown once a day when you lock or close after your
  // wrap-up time and today's wrap-up isn't done.
  import { focusNow } from '../lib/focus';
  import { fmtMinutes } from '../lib/format';
  import { answerLeave, leave } from '../lib/leave.svelte';

  const clock = $derived(leave.at.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }));
  const hours = $derived.by(() => {
    const o = leave.offer;
    if (!o) return '';
    return o.target > 0 ? `${fmtMinutes(o.logged)} of ${fmtMinutes(o.target)}` : fmtMinutes(o.logged);
  });
  const goWord = $derived(leave.how === 'close' ? 'Just close' : 'Just lock');

  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      void answerLeave('stay');
    }
  }
</script>

{#if leave.open && leave.offer}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={() => void answerLeave('stay')}></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="box paper" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="leave-title" onkeydown={keys}>
    <h2 id="leave-title">Wrap up before you go?</h2>
    <p class="say">
      It's {clock} and you've logged <strong>{hours}</strong> today. Two minutes now saves the morning.
    </p>
    <div class="row">
      <button type="button" class="btn" onclick={() => void answerLeave('not-today')}>Not today</button>
      <button type="button" class="btn" onclick={() => void answerLeave('go')}>{goWord}</button>
      <button type="button" class="btn primary" use:focusNow onclick={() => void answerLeave('wrapup')}>Wrap up</button>
    </div>
    <p class="hint">Asked once a day, after your wrap-up time, until the wrap-up is done.</p>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 86;
    background: color-mix(in srgb, var(--ink) 22%, transparent);
  }

  .box {
    position: fixed;
    z-index: 87;
    left: 50%;
    top: 26%;
    transform: translateX(-50%);
    width: min(440px, calc(100% - 32px));
    padding: 18px 20px 14px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h2 {
    margin: 0;
    font: 700 19px var(--display);
  }

  .say {
    margin: 0;
    font-size: 14.5px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }
</style>
