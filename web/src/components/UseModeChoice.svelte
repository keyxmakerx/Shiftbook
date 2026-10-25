<script lang="ts">
  // Three big buttons: Work, Home, Both (round 15). Used at first run and in Settings → Look.
  import type { UseMode } from '../lib/api';

  let { mode = $bindable(), onpick }: { mode: UseMode; onpick?: (m: UseMode) => void } = $props();

  const CHOICES: { id: UseMode; title: string; what: string }[] = [
    {
      id: 'work',
      title: 'Work',
      what: 'Hours against a daily target, tickets, the Timesheet. Shiftbook as it has always been.',
    },
    {
      id: 'home',
      title: 'Home',
      what: "Chores, bills, appointments and things you're chasing. No hours to fill, no Timesheet.",
    },
    {
      id: 'both',
      title: 'Both',
      what: 'Work and home on one Desk: the Timesheet and hours ring stay, and you can tick things done without a time.',
    },
  ];
</script>

<div class="choices" role="radiogroup" aria-label="What's Shiftbook for?">
  {#each CHOICES as c (c.id)}
    <button
      type="button"
      class="choice"
      role="radio"
      aria-checked={mode === c.id}
      onclick={() => {
        mode = c.id;
        onpick?.(c.id);
      }}
    >
      <b>{c.title}</b>
      <small>{c.what}</small>
    </button>
  {/each}
</div>

<style>
  .choices {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }

  @media (max-width: 620px) {
    .choices {
      grid-template-columns: 1fr;
    }
  }

  .choice {
    border: 1.5px solid var(--rule);
    border-radius: 10px;
    padding: 14px;
    background: var(--paper);
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: inherit;
    display: flex;
    flex-direction: column;
    gap: 4px;
    transition:
      border-color 0.2s,
      box-shadow 0.2s;
  }

  .choice[aria-checked='true'] {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .choice b {
    font: 600 17px var(--display);
  }

  .choice small {
    color: var(--ink-2);
    font-size: 12.5px;
    line-height: 1.4;
  }
</style>
