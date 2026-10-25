<script lang="ts">
  // Due soon on the Desk (round 15): repeating things that are overdue, due soon or need a new
  // expiry date, each with Done (or New date…) and Open. Hidden when nothing needs doing.
  import { markDone, needsAttention, openRepeating, refreshRepeating, repeating } from '../lib/repeating.svelte';
  import { app, setView } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const due = $derived(needsAttention(repeating.items).slice(0, 5));
  const more = $derived(needsAttention(repeating.items).length - due.length);

  // On the Desk's first showing, and again when a new day makes new things due.
  $effect(() => {
    void app.today;
    void refreshRepeating();
  });
</script>

{#if due.length > 0}
  <section class="paper due" aria-label="Due soon">
    <h3><Icon name="repeat" size={16} /> Due soon</h3>
    <ul>
      {#each due as r (r.id)}
        <li class:late={r.state === 'overdue'}>
          <span class="t">
            <b>{r.name}</b>
            <small>{r.state === 'renewed' ? 'put in the new expiry date' : r.dueWords}</small>
          </span>
          {#if r.state === 'renewed'}
            <button type="button" class="btn" onclick={() => openRepeating(r.id, true)}>New date…</button>
          {:else}
            <button type="button" class="btn primary" onclick={() => void markDone(r.id, r.name)}>✓ Done</button>
            <button type="button" class="ib" aria-label="Open {r.name}" title="Open" onclick={() => openRepeating(r.id)}>
              <Icon name="open" size={14} />
            </button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if more > 0}
      <button type="button" class="linkish" onclick={() => setView('repeating')}>and {more} more…</button>
    {/if}
  </section>
{/if}

<style>
  .due {
    padding: 12px 14px;
  }

  h3 {
    margin: 0 0 8px;
    display: flex;
    align-items: center;
    gap: 6px;
    font: 650 15px var(--display);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--rule);
  }

  li:last-child {
    border-bottom: 0;
  }

  .t {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .t b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  small {
    color: var(--mut);
    font-size: 12px;
  }

  li.late small {
    color: var(--bad);
    font-weight: 600;
  }

  .ib {
    border: 0;
    background: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: var(--mut);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .linkish {
    border: 0;
    background: none;
    padding: 6px 0 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    font-size: 13px;
  }
</style>
