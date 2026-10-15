<script lang="ts">
  // The Done tickets tray: paste tickets as you finish them through the day. Each one adds
  // its minutes (15 by default, change with the stepper) and they all roll up into the
  // day's "Tickets" entry (the rule lives in crates/core/src/tray.rs).
  import { api } from '../lib/api';
  import type { TrayDay, TrayItem } from '../lib/api';
  import { fmtMinutes } from '../lib/format';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { copyText } from '../lib/protection.svelte';
  import { app, attempt, flashEntry, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  import TicketChip from './TicketChip.svelte';

  const STEP = 5;
  let text = $state('');

  async function apply(result: TrayDay | undefined) {
    if (!result) return;
    app.tray = result;
    await refresh();
    if (result.entryId) flashEntry(result.entryId);
  }

  async function add(pasted: string) {
    if (!pasted.trim()) return;
    const before = app.tray?.items.length ?? 0;
    const result = await attempt(() => api.trayAdd(app.selected, pasted));
    if (!result) return;
    text = '';
    const added = result.items.length - before;
    await apply(result);
    toast(added > 0 ? `Added ${added} ticket${added === 1 ? '' : 's'} to the tray` : 'Those tickets are already in the tray.');
  }

  async function step(item: TrayItem, by: number) {
    const next = Math.max(0, Math.min(480, item.minutes + by));
    if (next === item.minutes) return;
    await apply(await attempt(() => api.traySetMinutes(item.id, next)));
  }

  async function remove(item: TrayItem) {
    const result = await attempt(() => api.trayRemove(item.id));
    if (!result) return;
    await apply(result);
    toast(`Took ${item.ticket} out of the tray`, 'Undo', () => {
      void attempt(() => api.trayRestore(item.id)).then(apply);
    });
  }

  function itemMenu(item: TrayItem): MenuSpec {
    return {
      sections: [
        {
          head: `${item.ticket} · ${fmtMinutes(item.minutes)}`,
          items: [
            { label: `Add ${STEP} minutes`, icon: 'plus', side: true, act: () => void step(item, STEP) },
            { label: `Take away ${STEP} minutes`, icon: 'minus', side: true, act: () => void step(item, -STEP) },
            {
              label: 'Copy ticket number',
              icon: 'copy',
              top: true,
              act: () => void copyText(item.ticket).then((ok) => toast(ok ? `Copied ${item.ticket}` : "Couldn't copy here.")),
            },
            ...item.links.map((l) => ({
              label: `Copy ${l.fieldName} link`,
              icon: 'link' as const,
              top: true,
              act: () => void copyText(l.url).then((ok) => toast(ok ? 'Copied the link' : "Couldn't copy here.")),
            })),
          ],
        },
        { items: [{ label: 'Take out of the tray', icon: 'trash', danger: true, side: true, act: () => void remove(item) }] },
      ],
    };
  }
</script>

<article class="paper card">
  <h3>
    <span class="title"><Icon name="tray" size={16} /> Done tickets</span>
    <span class="total mono">{app.tray?.hours ?? '0'} h</span>
  </h3>
  <p class="hint">Paste tickets as you finish them. They add up to one “Tickets” entry for the day.</p>
  <input
    class="text-in"
    bind:value={text}
    placeholder="Paste one or many tickets, then Enter"
    autocomplete="off"
    spellcheck="false"
    aria-label="Add finished tickets"
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        void add(text);
      }
    }}
    onpaste={(e) => {
      const pasted = e.clipboardData?.getData('text') ?? '';
      if (/[\n,;\t]/.test(pasted.trim())) {
        e.preventDefault();
        void add(text + ' ' + pasted);
      }
    }}
  />
  {#if app.tray && app.tray.items.length > 0}
    <ul class="items">
      {#each app.tray.items as item, i (item.id)}
        <li class="item rise" style:--i={i} use:paperMenu={() => itemMenu(item)} tabindex="-1">
          <TicketChip
            number={item.ticket}
            links={item.links}
            color="var(--accent)"
            onRemove={() => void remove(item)}
            removeLabel="Take out of the tray"
          />
          <span class="added">+{fmtMinutes(item.minutes)}</span>
          <span class="stepper" role="group" aria-label="Minutes for {item.ticket}">
            <button type="button" aria-label="{STEP} minutes less" onclick={() => step(item, -STEP)} disabled={item.minutes === 0}>
              <Icon name="minus" size={13} />
            </button>
            <span class="mono val">{item.minutes}</span>
            <button type="button" aria-label="{STEP} minutes more" onclick={() => step(item, STEP)}>
              <Icon name="plus" size={13} />
            </button>
          </span>
          <button type="button" class="x" aria-label="Take {item.ticket} out of the tray" onclick={() => remove(item)}>
            <Icon name="x" size={14} />
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty-note">Nothing in the tray yet for this day.</p>
  {/if}
</article>

<style>
  .card {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 3px solid var(--accent);
  }

  h3 {
    margin: 0;
    font: 600 14px var(--ui);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .title {
    display: inline-flex;
    gap: 7px;
    align-items: center;
  }

  .total {
    font-weight: 700;
    color: var(--accent);
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 8px;
    padding: 5px 4px;
    border-top: 1px solid var(--rule);
    border-radius: 6px;
    outline-offset: -2px;
  }

  .item:first-child {
    border-top: 0;
  }

  .item :global(.chip) {
    justify-self: start;
  }

  .added {
    font: 600 11.5px var(--mono);
    color: var(--ok);
  }

  .stepper {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--rule);
    border-radius: 7px;
    background: var(--paper);
  }

  .stepper button,
  .x {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    cursor: pointer;
    border-radius: 6px;
  }

  .stepper button:hover:not(:disabled),
  .x:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink);
  }

  .stepper button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .val {
    min-width: 26px;
    text-align: center;
    font-size: 12px;
    font-weight: 700;
  }
</style>
