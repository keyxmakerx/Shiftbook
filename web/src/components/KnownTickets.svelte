<script lang="ts">
  // A ticket Shiftbook already knows (round 15, card r15-things): pasted into Log work or the
  // Tickets tray, it shows since when it's been worked, the time on it so far, where its time
  // goes and the first line of its notepad, so it's clearly the same ticket as before.
  import { api } from '../lib/api';
  import type { ItemSummary } from '../lib/api';
  import { fmtMinutes, relativeDay } from '../lib/format';
  import { repeating } from '../lib/repeating.svelte';
  import { app, projectById } from '../lib/store.svelte';

  let {
    numbers,
    before = null,
  }: {
    numbers: string[];
    /** Only tickets first worked before this day (the tray: the row just pasted is new). */
    before?: string | null;
  } = $props();

  let known = $state<ItemSummary[]>([]);

  $effect(() => {
    const list = numbers;
    void before;
    if (list.length === 0) {
      known = [];
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api
        .items(list)
        .then((items) => {
          if (live) known = items.filter((i) => i.first && (!before || i.first < before));
        })
        .catch(() => {
          if (live) known = [];
        });
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  function line(i: ItemSummary): string {
    const parts: string[] = [];
    if (i.first) parts.push(`${i.done ? 'done, worked since' : 'open since'} ${relativeDay(i.first, app.today)}`);
    if (i.minutes) parts.push(`${fmtMinutes(i.minutes)} so far`);
    const home = projectById(i.projectId)?.name ?? repeating.items.find((r) => r.id === i.recurringId)?.name;
    if (home) parts.push(home);
    const note = i.note?.split('\n').find((l) => l.trim());
    if (note) parts.push(`notepad: “${note.trim()}”`);
    return parts.join(' · ');
  }
</script>

{#if known.length > 0}
  <ul class="known" aria-label="Tickets you've worked before">
    {#each known as i (i.id)}
      <li><b class="mono">{i.number}</b> {line(i)}</li>
    {/each}
  </ul>
{/if}

<style>
  .known {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .known b {
    color: var(--ink);
    margin-right: 4px;
  }

  li {
    overflow-wrap: anywhere;
  }
</style>
