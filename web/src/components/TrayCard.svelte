<script lang="ts">
  // The Tickets tray: paste tickets as you work on them through the day. Each one adds its
  // minutes (15 by default; a little box asks how long so far, and the stepper changes it).
  // A ticket starts still open and comes back each day until it is ticked done (round 14). Tickets without a project roll up
  // into the day's "Tickets" entry, the others into their project's (rules in
  // crates/core/src/tray.rs). Click a ticket for its notepad and options.
  import { api } from '../lib/api';
  import type { TrayItem } from '../lib/api';
  import { dayMonth, fmtMinutes, weekdayShort } from '../lib/format';
  import { openMenu, paperMenu } from '../lib/menu.svelte';
  import { closePanel, openNotepad, panel, togglePanel } from '../lib/panel.svelte';
  import { copyText } from '../lib/protection.svelte';
  import { app, attempt, projectById, projectColor, setView, words } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { applyTray, carryTray, removeTray, setTrayDone, trayMenu } from '../lib/trayActions';
  import Icon from './Icon.svelte';
  import ItemPanel from './ItemPanel.svelte';
  import TicketChip from './TicketChip.svelte';

  const STEP = 5;
  /** Choices in the "time on it so far?" box. */
  const SO_FAR = [5, 15, 30, 45, 60, 120];
  let text = $state('');
  /** Tickets just added, waiting for "time on it so far?". */
  let asking = $state<string[]>([]);

  async function add(pasted: string) {
    if (!pasted.trim()) return;
    const before = new Set(app.tray?.items.map((i) => i.id) ?? []);
    const result = await attempt(() => api.trayAdd(app.selected, pasted));
    if (!result) return;
    text = '';
    const added = result.items.filter((i) => !before.has(i.id)).map((i) => i.id);
    await applyTray(result);
    if (added.length) asking = added;
    else toast('Those tickets are already in the tray.');
  }

  /** Answer "time on it so far?" for one ticket. */
  async function soFar(item: TrayItem, minutes: number) {
    asking = asking.filter((id) => id !== item.id);
    if (minutes !== item.minutes) await applyTray(await attempt(() => api.traySetMinutes(item.id, minutes)));
  }

  const askingItems = $derived(asking.map((id) => app.tray?.items.find((i) => i.id === id)).filter((i): i is TrayItem => !!i));

  async function step(item: TrayItem, by: number) {
    const next = Math.max(0, Math.min(480, item.minutes + by));
    if (next === item.minutes) return;
    await applyTray(await attempt(() => api.traySetMinutes(item.id, next)));
  }

  // A one-line note is typed right in the row and saved when you leave the box or press
  // Enter; a longer one lives in the notepad under the ticket.
  async function saveNote(item: TrayItem, text: string) {
    if (text.trim() === (item.note ?? '')) return;
    const result = await attempt(() => api.traySetNote(item.id, text));
    if (result) app.tray = result;
  }

  const oneLine = (note: string | null) => !note || !note.includes('\n');

  const keyOf = (item: TrayItem) => `tray:${item.id}`;
  const isOpen = (item: TrayItem) => panel.open === keyOf(item);
  const toggle = (item: TrayItem, focus = false) => togglePanel(keyOf(item), focus);

  function rowClick(e: MouseEvent, item: TrayItem) {
    if ((e.target as HTMLElement).closest('input, button, a')) return;
    toggle(item);
  }

  // Saved responses: a quick menu of messages to paste into tickets. Picking one copies it;
  // nothing is matched to a ticket (owner's call, round 8).
  function editResponses() {
    app.settingsSection = 'responses';
    setView('settings');
  }

  async function openResponses(anchor: HTMLElement) {
    const list = await attempt(() => api.listResponses());
    if (!list) return;
    const copy = (name: string, text: string) =>
      void copyText(text).then((ok) => toast(ok ? `Copied “${name}”` : "Couldn't copy here."));
    openMenu(anchor, {
      sections: [
        list.length > 0
          ? {
              head: 'Copy a response',
              items: list.map((r) => ({ label: r.name, icon: 'copy' as const, act: () => copy(r.name, r.text) })),
            }
          : { head: 'No saved responses yet', items: [{ label: 'Add your first response', icon: 'plus', act: editResponses }] },
        { items: [{ label: 'Edit responses', icon: 'edit', top: true, act: editResponses }] },
      ],
    });
  }

  const menuFor = (item: TrayItem) => trayMenu(item, () => openNotepad(keyOf(item)));
</script>

<article class="paper card">
  <h3>
    <span class="title"><Icon name="tray" size={16} /> {words().tickets}</span>
    <span class="right">
      <button
        type="button"
        class="resp-btn"
        title="Copy a saved response"
        aria-label="Copy a saved response"
        onclick={(e) => void openResponses(e.currentTarget as HTMLElement)}
      >
        <Icon name="copy" size={15} />
      </button>
      <span class="total mono">{app.tray?.hours ?? '0'} h</span>
    </span>
  </h3>
  <p class="hint">Paste tickets as you work on them. Each stays open, and comes back tomorrow, until you tick it done.</p>
  <input
    class="text-in"
    bind:value={text}
    placeholder="Paste one or many tickets, then Enter"
    autocomplete="off"
    spellcheck="false"
    aria-label="Add tickets"
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
  {#if askingItems.length}
    <div class="ask" role="group" aria-label="Time on the new tickets so far">
      <span class="ask-head">
        <b>Time on {askingItems.length === 1 ? 'it' : 'them'} so far?</b>
        <button type="button" class="ask-skip" onclick={() => (asking = [])}>Keep {fmtMinutes(askingItems[0]!.minutes)}</button>
      </span>
      {#each askingItems as item (item.id)}
        <span class="ask-row">
          {#if askingItems.length > 1}<span class="mono ask-tk">{item.ticket}</span>{/if}
          <span class="picks">
            {#each SO_FAR as m (m)}
              <button type="button" class:on={item.minutes === m} onclick={() => void soFar(item, m)}>{fmtMinutes(m)}</button>
            {/each}
          </span>
        </span>
      {/each}
    </div>
  {/if}
  {#if app.tray && app.tray.items.length > 0}
    <ul class="items">
      {#each app.tray.items as item, i (item.id)}
        {@const project = projectById(item.projectId)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <li
          class="item rise"
          class:still={!item.done}
          class:opened={isOpen(item)}
          style:--i={i}
          use:paperMenu={() => menuFor(item)}
          tabindex="-1"
          onclick={(e) => rowClick(e, item)}
        >
          <button
            type="button"
            class="tick"
            class:done={item.done}
            title={item.done ? 'Done. Click if it is still open.' : 'Still open. Click when it is done.'}
            aria-label={item.done ? `${item.ticket} is done; mark it still open` : `${item.ticket} is still open; mark it done`}
            aria-pressed={item.done}
            onclick={() => void setTrayDone(item, !item.done)}
          >
            {#if item.done}<Icon name="check" size={12} />{/if}
          </button>
          <TicketChip
            number={item.ticket}
            links={item.links}
            color={project ? projectColor(item.projectId) : 'var(--accent)'}
            onRemove={() => void removeTray(item)}
            removeLabel="Take out of the tray"
          />
          {#if oneLine(item.note)}
            <input
              class="note"
              value={item.note ?? ''}
              title={item.note ?? ''}
              placeholder={item.done ? 'A few words…' : 'Still open: what is left?'}
              autocomplete="off"
              aria-label="Note about {item.ticket}"
              onchange={(e) => void saveNote(item, (e.currentTarget as HTMLInputElement).value)}
              onkeydown={(e) => {
                if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
              }}
            />
          {:else}
            <button type="button" class="note more" title="Open the notepad" onclick={() => toggle(item, true)}>
              {item.note?.split('\n')[0]} …
            </button>
          {/if}
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
          <button type="button" class="x" aria-label="Notepad and options for {item.ticket}" aria-expanded={isOpen(item)} onclick={() => toggle(item)}>
            <Icon name="more" size={14} />
          </button>
          {#if !item.done || project}
            <span class="tags">
              {#if !item.done}<span class="tag open">Still open</span>{/if}
              {#if project}<span class="tag" style:--c={projectColor(item.projectId)}>{project.name}</span>{/if}
            </span>
          {/if}
        </li>
        {#if isOpen(item)}
          <li class="under">
            <ItemPanel tray={item} menu={() => menuFor(item)} focusPad={panel.focusPad} onclose={closePanel} />
          </li>
        {/if}
      {/each}
    </ul>
  {:else}
    <p class="empty-note">Nothing in the tray yet for this day.</p>
  {/if}
  {#if app.tray && app.tray.carried.length > 0}
    <div class="carried">
      <span class="lbl">Still open from before</span>
      <ul class="items">
        {#each app.tray.carried as item (item.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
          <li class="item still" class:opened={isOpen(item)} use:paperMenu={() => menuFor(item)} tabindex="-1" onclick={(e) => rowClick(e, item)}>
            <button
              type="button"
              class="tick"
              title="Still open. Click when it is done."
              aria-label="{item.ticket} is still open; mark it done"
              aria-pressed="false"
              onclick={() => void setTrayDone(item, true)}
            ></button>
            <TicketChip number={item.ticket} links={item.links} color="var(--warn)" />
            <span class="note faint" title={item.note ?? ''}>{item.note?.split('\n')[0] ?? ''}</span>
            <span class="since">{weekdayShort(item.date)} {dayMonth(item.date)}</span>
            <button type="button" class="today" title="Worked on it again: add it to this day with 15 minutes" onclick={() => void carryTray(item)}>
              <Icon name="plus" size={12} /> {app.selected === app.today ? 'Today' : 'This day'}
            </button>
          </li>
          {#if isOpen(item)}
            <li class="under">
              <ItemPanel tray={item} menu={() => menuFor(item)} focusPad={panel.focusPad} onclose={closePanel} />
            </li>
          {/if}
        {/each}
      </ul>
    </div>
  {/if}
</article>

<style>
  .card {
    container-type: inline-size;
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

  .right {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .resp-btn {
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink-2);
    width: 30px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    cursor: pointer;
  }

  .resp-btn:hover {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
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

  .ask {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px 10px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 10%, var(--paper));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    animation: ask-in calc(0.25s * var(--m)) var(--spring);
  }

  @keyframes ask-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }

  .ask-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 13px;
  }

  .ask-skip {
    border: 0;
    background: none;
    padding: 2px 4px;
    font: 600 12px var(--ui);
    color: var(--mut);
    cursor: pointer;
  }

  .ask-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .ask-tk {
    font-size: 12px;
    min-width: 92px;
  }

  .picks {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .picks button {
    min-width: 40px;
    padding: 4px 8px;
    border: 1px solid var(--rule);
    border-radius: 7px;
    background: var(--paper);
    font: 600 12px var(--mono);
    color: var(--ink);
    cursor: pointer;
  }

  .picks button:hover,
  .picks button.on {
    border-color: var(--accent);
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
    grid-template-columns: auto auto minmax(0, 1fr) auto auto auto;
    cursor: pointer;
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

  .item.opened {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border-radius: 6px 6px 0 0;
  }

  .under {
    margin: -2px 0 6px;
  }

  .tick {
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 50%;
    border: 2px dashed var(--warn);
    background: none;
    color: #fff;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .tick.done {
    border: 2px solid var(--ok);
    background: var(--ok);
  }

  .tick:hover {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 20%, transparent);
  }

  .tags {
    grid-column: 3 / -1;
    display: flex;
    gap: 6px;
    margin-top: -2px;
  }

  .tag {
    white-space: nowrap;
    font: 700 10px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 5px;
    color: var(--c, var(--ink-2));
    background: color-mix(in srgb, var(--c, var(--ink)) 10%, transparent);
  }

  .tag.open {
    color: var(--warn);
    background: var(--warn-wash);
  }

  .note.more {
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .note.faint {
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .carried {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 6px;
    border-top: 1px dashed color-mix(in srgb, var(--warn) 45%, transparent);
  }

  .carried .item {
    grid-template-columns: auto auto minmax(0, 1fr) auto auto;
  }

  .lbl {
    font: 700 10px var(--ui);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--warn);
  }

  .since {
    font-size: 11.5px;
    color: var(--mut);
    white-space: nowrap;
  }

  /* In the Desk's side panel: tick, ticket, stepper and options on one line, the note under them. */
  @container (max-width: 380px) {
    .added,
    .since {
      display: none;
    }

    .item,
    .carried .item {
      grid-template-columns: auto minmax(0, 1fr) auto auto;
      gap: 2px 6px;
    }

    .item :global(.chip) {
      grid-column: 2;
      grid-row: 1;
    }

    .note {
      grid-column: 2 / -1;
      grid-row: 2;
      padding: 2px 7px;
    }

    .tags {
      grid-column: 2 / -1;
      grid-row: 3;
    }
  }

  .today {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: 600 12px var(--ui);
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
  }

  .today:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .item :global(.chip) {
    justify-self: start;
  }

  .note {
    min-width: 0;
    font: 13px var(--ui);
    color: var(--ink);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 4px 7px;
  }

  .note::placeholder {
    color: var(--mut);
    opacity: 0.7;
  }

  .note:hover {
    border-color: var(--rule);
  }

  .note:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: var(--paper);
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
