<script lang="ts">
  // One entry on Day: hours, title, project, tickets and quick +time buttons.
  // Click slides out its notepad and options underneath (round 12); double-click or Enter
  // opens the details drawer; right-click opens its paper menu with everything.
  // On a phone (round 15): swipe right for +15 minutes, left for Timer and Delete.
  import { api } from '../lib/api';
  import type { Entry } from '../lib/api';
  import { addEntryTime, canTime as canTimeEntry, entryMenu, removeEntry } from '../lib/entryActions';
  import { phone } from '../lib/phone.svelte';
  import { isSideways, slide, SWIPE_DONE_PX, SWIPE_MAX_PX, SWIPE_START_PX, swipeEnd } from '../lib/swipe';
  import { paperMenu } from '../lib/menu.svelte';
  import { closePanel, panel, togglePanel } from '../lib/panel.svelte';
  import { app, attempt, projectById, projectColor, putEntry, refresh, words } from '../lib/store.svelte';
  import { hasWork } from '../lib/words';
  import { toast } from '../lib/toast.svelte';
  import { startTimer, stopTimer, timer } from '../lib/timer.svelte';
  import ItemPanel from './ItemPanel.svelte';
  import TicketChip from './TicketChip.svelte';

  let { entry, index = 0 }: { entry: Entry; index?: number } = $props();

  let row: HTMLElement | undefined = $state();
  const project = $derived(projectById(entry.projectId));
  const color = $derived(projectColor(entry.projectId));
  // The optional timer (Settings → Your day) runs on today's entries only.
  const canTime = $derived(canTimeEntry(entry));
  const timing = $derived(timer.running && timer.target?.entryId === entry.id);

  $effect(() => {
    if (app.flash.id !== entry.id || !row) return;
    void app.flash.serial;
    row.classList.remove('flash');
    void row.offsetWidth;
    row.classList.add('flash');
  });

  const key = $derived(`entry:${entry.id}`);
  const open = $derived(panel.open === key);
  const firstLine = (text: string) => text.split('\n')[0];
  const addTime = (minutes: number) => addEntryTime(entry, minutes);
  const menu = () => entryMenu(entry);

  // Swiping (phone only; not the Tickets tray entry, whose time comes from the tray).
  const swipes = $derived(phone.on && !entry.isTray);
  let dx = $state(0);
  let dragging = $state(false);
  let revealed = $state(false);
  let start: { x: number; y: number } | null = null;
  let sideways = false;
  let swiped = false;

  function swipeDown(e: PointerEvent) {
    if (!swipes || (e.target as HTMLElement).closest('button, a')) return;
    start = { x: e.clientX, y: e.clientY };
    sideways = false;
  }

  function swipeMove(e: PointerEvent) {
    if (!start) return;
    const mx = e.clientX - start.x;
    const my = e.clientY - start.y;
    if (!sideways) {
      if (!isSideways(mx, my)) {
        // Scrolling the list, not swiping.
        if (Math.abs(my) >= SWIPE_START_PX) start = null;
        return;
      }
      sideways = true;
      dragging = true;
      row?.setPointerCapture(e.pointerId);
    }
    dx = slide(mx - (revealed ? SWIPE_MAX_PX : 0));
  }

  function swipeUp() {
    if (!start) return;
    start = null;
    if (!sideways) return;
    swiped = true;
    dragging = false;
    const end = swipeEnd(dx);
    dx = 0;
    if (end === 'add' && !revealed) void addTime(15);
    revealed = end === 'reveal';
  }

  async function removeTicket(ticketId: string, number: string) {
    const updated = await attempt(() => api.removeTicket(ticketId));
    if (!updated) return;
    putEntry(updated);
    toast(`Removed ${number}`, 'Undo', () => {
      void attempt(() => api.addEntryTickets(updated.id, number)).then(refresh);
    });
  }
</script>

<div class="swipe" class:on={swipes}>
{#if swipes && (dx !== 0 || revealed)}
  <div class="behind">
    <span class="add" class:ready={dx >= SWIPE_DONE_PX}>+15 min</span>
    {#if revealed}
      <span class="acts">
        {#if canTime}
          <button
            type="button"
            onclick={() => {
              revealed = false;
              void (timing ? stopTimer() : startTimer(entry));
            }}>{timing ? 'Stop' : 'Timer'}</button
          >
        {/if}
        <button
          type="button"
          class="del"
          onclick={() => {
            revealed = false;
            void removeEntry(entry);
          }}>Delete</button
        >
      </span>
    {/if}
  </div>
{/if}
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="row rise"
  class:tray={entry.isTray}
  class:opened={open}
  class:settle={!dragging}
  style:transform={swipes && (dx !== 0 || revealed) ? `translateX(${dragging ? dx : -SWIPE_MAX_PX}px)` : null}
  data-entry-row
  style:--c={color}
  style:--i={index}
  bind:this={row}
  role="button"
  tabindex="0"
  aria-expanded={open}
  title={phone.on ? undefined : "Click for the notepad and options · double-click to open · right-click for everything"}
  onpointerdown={swipeDown}
  onpointermove={swipeMove}
  onpointerup={swipeUp}
  onpointercancel={swipeUp}
  onclick={(e) => {
    if (swiped) {
      // The click a finished swipe makes.
      swiped = false;
      return;
    }
    if (revealed) {
      // A tap closes the Timer and Delete buttons.
      revealed = false;
      return;
    }
    if ((e.target as HTMLElement).closest('button, a')) return;
    togglePanel(key);
  }}
  ondblclick={() => {
    closePanel();
    app.drawer = entry.id;
  }}
  onkeydown={(e) => {
    if (e.target !== row) return;
    if (e.key === 'Enter') app.drawer = entry.id;
    else if (e.key === ' ') {
      e.preventDefault();
      togglePanel(key);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      // Up and down move between entries; the time buttons show on the focused one.
      e.preventDefault();
      const rows = [...(row?.closest('.list')?.querySelectorAll<HTMLElement>('[data-entry-row]') ?? [])];
      rows[rows.indexOf(row!) + (e.key === 'ArrowDown' ? 1 : -1)]?.focus();
    }
  }}
  use:paperMenu={menu}
>
  {#if entry.minutes === 0 && !entry.isTray}
    <!-- Done without time (round 15): a tick, not "0h". -->
    <span class="h done" title="Done">✓</span>
  {:else}
    <span class="h">{entry.hours}</span>
  {/if}
  <span class="t">
    <b>{entry.description || 'Untitled'}</b>
    <small>
      {project?.name ?? (hasWork(app.prefs?.useMode) ? 'No project' : 'Done')}{#if entry.isTray}{` · from the ${words().tickets} tray`}{/if}{#if entry.notes}{` · ✎ ${firstLine(entry.notes)}`}{/if}
    </small>
  </span>
  <span class="chips">
    {#each entry.tickets as t (t.id)}
      <TicketChip
        number={t.number}
        links={t.links}
        {color}
        onRemove={entry.isTray ? null : () => void removeTicket(t.id, t.number)}
      />
    {/each}
  </span>
  <span class="plus">
    {#if canTime}
      <button
        type="button"
        class="go"
        class:on={timing}
        aria-label={timing ? 'Stop the timer' : 'Start the timer'}
        onclick={(e) => { e.stopPropagation(); void (timing ? stopTimer() : startTimer(entry)); }}
      >{timing ? '■ Stop' : '▶ Start'}</button>
    {/if}
    {#if !entry.isTray}
      <button type="button" onclick={(e) => { e.stopPropagation(); void addTime(15); }}>+15m</button>
      <button type="button" onclick={(e) => { e.stopPropagation(); void addTime(30); }}>+30m</button>
      <button type="button" onclick={(e) => { e.stopPropagation(); void addTime(60); }}>+1h</button>
    {/if}
  </span>
</div>
</div>
{#if open}
  <div class="under">
    <ItemPanel {entry} {menu} focusPad={panel.focusPad} onclose={closePanel} />
  </div>
{/if}

<style>
  .swipe {
    position: relative;
  }

  .swipe.on .row {
    touch-action: pan-y;
    user-select: none;
    -webkit-user-select: none;
    background: var(--paper);
  }

  .row.settle {
    transition:
      background 0.2s,
      transform 0.2s;
  }

  .behind {
    position: absolute;
    inset: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }

  .add {
    font: 650 13px var(--mono);
    color: var(--mut);
  }

  .add.ready {
    color: var(--ok);
  }

  .acts {
    display: flex;
    gap: 6px;
    width: 110px;
    justify-content: flex-end;
  }

  .acts button {
    border: 0;
    border-radius: 8px;
    min-height: 44px;
    padding: 0 10px;
    font-weight: 650;
    background: var(--ink);
    color: var(--paper);
  }

  .acts .del {
    background: var(--danger);
    color: #fff;
  }

  .row {
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr) auto auto;
    gap: 12px;
    align-items: center;
    padding: 10px;
    border-radius: 8px;
    transition: background 0.2s;
    cursor: pointer;
    outline-offset: -2px;
  }

  .row:hover {
    background: color-mix(in srgb, var(--ink) 4%, transparent);
  }

  .row.opened {
    background: color-mix(in srgb, var(--c) 9%, transparent);
    border-radius: 8px 8px 0 0;
  }

  .under {
    margin: 0 0 8px 54px;
  }

  .h {
    font: 700 15px var(--mono);
    color: var(--c);
    text-align: right;
    transition: color 0.4s;
  }

  .h.done {
    color: var(--ok);
    font-size: 18px;
  }

  .t {
    min-width: 0;
  }

  .t b {
    font-weight: 600;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .t small {
    color: var(--mut);
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chips {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    justify-content: flex-end;
    max-width: 280px;
  }

  .plus {
    opacity: 0;
    transition: opacity 0.2s;
    display: flex;
    gap: 4px;
  }

  .row:hover .plus,
  .row:focus-within .plus,
  .plus:has(.on) {
    opacity: 1;
  }

  .go.on {
    color: var(--accent);
    font-weight: 650;
  }

  .plus button {
    border: 1px solid var(--rule);
    background: var(--paper);
    border-radius: 6px;
    font: 600 11px var(--mono);
    padding: 3px 7px;
    cursor: pointer;
    color: var(--ink);
  }

  .plus button:hover {
    background: var(--ink);
    color: var(--paper);
  }

  .row:global(.flash) {
    animation: flash calc(1.2s * var(--m)) var(--ease);
  }

  @keyframes flash {
    0% {
      background: color-mix(in srgb, var(--c) 26%, transparent);
    }
    100% {
      background: transparent;
    }
  }

  @media (max-width: 760px) {
    .row {
      grid-template-columns: 46px minmax(0, 1fr);
    }
    .chips,
    .plus {
      grid-column: 2;
      justify-content: flex-start;
    }
    .plus {
      opacity: 1;
    }
  }
</style>
