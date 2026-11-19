<script lang="ts">
  // "Split in two…" (round 21): share an entry's time between two parts. The first keeps its
  // tickets and notepad; the second is a new entry on the same day.
  import { focusNow } from '../lib/focus';
  import { fmtMinutes } from '../lib/format';
  import { answerSplit, splitPrompt, splitStart } from '../lib/splitPrompt.svelte';
  import { app, projectById } from '../lib/store.svelte';
  import { repeating } from '../lib/repeating.svelte';

  const SAME = 'same';
  const NONE = 'none';

  let firstName = $state('');
  let secondName = $state('');
  let first = $state(1);
  let home = $state(SAME);

  const entry = $derived(splitPrompt.entry);
  const total = $derived(entry?.minutes ?? 0);
  const rest = $derived(total - first);
  const step = $derived(total > 10 ? 5 : 1);

  $effect(() => {
    void splitPrompt.serial;
    const e = splitPrompt.entry;
    if (!e) return;
    firstName = e.description;
    secondName = e.description;
    first = splitStart(e.minutes);
    home = SAME;
  });

  const sameName = $derived(
    entry?.projectId
      ? projectById(entry.projectId)?.name
      : entry?.recurringId
        ? repeating.items.find((r) => r.id === entry.recurringId)?.name
        : undefined,
  );
  const valid = $derived(Number.isInteger(first) && first >= 1 && rest >= 1 && firstName.trim() !== '' && secondName.trim() !== '');

  function nudge(by: number) {
    first = Math.min(total - 1, Math.max(1, first + by));
  }

  function ok() {
    if (!valid) return;
    answerSplit({
      firstMinutes: first,
      firstDescription: firstName.trim(),
      secondDescription: secondName.trim(),
      sameHome: home === SAME,
      secondProject: home === SAME || home === NONE ? null : home,
    });
  }

  function keys(e: KeyboardEvent) {
    if (e.key === 'Enter') ok();
    else if (e.key === 'Escape') {
      e.stopPropagation();
      answerSplit(null);
    }
  }
</script>

{#if splitPrompt.open && entry}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={() => answerSplit(null)}></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="box paper" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="split-title" onkeydown={keys}>
    <h2 id="split-title">Split in two</h2>
    <p class="hint">
      {fmtMinutes(total)} to share. The first part keeps the tickets and the notepad.
    </p>
    <div class="part">
      <label class="lbl" for="split-first">First part</label>
      <input id="split-first" class="text-in" bind:value={firstName} use:focusNow autocomplete="off" />
      <span class="mins" role="group" aria-label="Time for the first part">
        <button type="button" class="hbtn" aria-label="{step} minutes less" disabled={first <= 1} onclick={() => nudge(-step)}>−</button>
        <b class="mono">{fmtMinutes(first)}</b>
        <button type="button" class="hbtn" aria-label="{step} minutes more" disabled={rest <= 1} onclick={() => nudge(step)}>+</button>
      </span>
    </div>
    <div class="part">
      <label class="lbl" for="split-second">Second part</label>
      <input id="split-second" class="text-in" bind:value={secondName} autocomplete="off" />
      <span class="mins"><b class="mono">{fmtMinutes(rest)}</b></span>
    </div>
    <label class="proj">
      <span class="lbl">Second part counts under</span>
      <select class="text-in" bind:value={home}>
        <option value={SAME}>The same as the first{sameName ? ` (${sameName})` : ''}</option>
        {#each app.projects.filter((p) => !p.archived && p.id !== entry.projectId) as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
        {#if sameName}<option value={NONE}>No project</option>{/if}
      </select>
    </label>
    <div class="row">
      <button type="button" class="btn" onclick={() => answerSplit(null)}>Cancel</button>
      <button type="button" class="btn primary" disabled={!valid} onclick={ok}>Split in two</button>
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
    top: 24%;
    transform: translateX(-50%);
    width: min(460px, calc(100% - 32px));
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

  .part {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px 8px;
    align-items: center;
  }

  .part .lbl {
    grid-column: 1 / -1;
  }

  .mins {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    min-width: 128px;
  }

  .mins b {
    min-width: 58px;
    text-align: center;
  }

  .proj {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
