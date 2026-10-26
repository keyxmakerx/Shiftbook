<script lang="ts">
  // The slide-out under an entry or a ticket (round 12): one click on a row on Day, a card on
  // the Desk or a ticket in the tray opens it right underneath. It holds the everyday things:
  // the notepad (saved as you type), the entry's tickets, the project, Follow up, done
  // or still open (tickets), more time and, only when the timer is turned on, Start. The full
  // menu stays on right-click and on "More". Esc folds it away.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { Entry, TrayItem } from '../lib/api';
  import { bringUpSection, followWords } from '../lib/bringUp';
  import { addEntryTime, canTime, moveEntryTo, moveEntryToRepeating } from '../lib/entryActions';
  import { openMenu, type MenuSpec } from '../lib/menu.svelte';
  import { app, attempt, projectById, projectColor, putEntry, refresh } from '../lib/store.svelte';
  import { repeating, repeatingChoices } from '../lib/repeating.svelte';
  import { setTrayDone, setTrayProject, setTrayRepeating } from '../lib/trayActions';
  import { openWindow } from '../lib/windows.svelte';
  import { startTimer, stopTimer, timer } from '../lib/timer.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  import TicketChip from './TicketChip.svelte';

  let {
    entry = null,
    tray = null,
    menu,
    onclose,
    onsaved,
    focusPad = false,
  }: {
    entry?: Entry | null;
    tray?: TrayItem | null;
    /** The full paper menu, for "More". */
    menu: () => MenuSpec;
    onclose: () => void;
    /** The notepad was saved (the Desk reloads, as its cards may be from other days). */
    onsaved?: () => void;
    /** Put the cursor in the notepad as it opens. */
    focusPad?: boolean;
  } = $props();

  const SAVE_AFTER_MS = 700;

  // The notepad keeps its own draft so a reload elsewhere never resets what is being typed.
  let draft = $state('');
  let saved = $state('');
  let saving: ReturnType<typeof setTimeout> | null = null;
  let status = $state<'' | 'saving' | 'saved'>('');
  let ticketText = $state('');
  let pad: HTMLTextAreaElement | undefined = $state();

  onMount(() => {
    draft = saved = (entry ? entry.notes : tray?.note) ?? '';
    if (focusPad) requestAnimationFrame(() => pad?.focus());
    return () => {
      if (saving) {
        clearTimeout(saving);
        void save();
      }
    };
  });

  const projectId = $derived(entry ? entry.projectId : (tray?.projectId ?? null));
  const project = $derived(projectById(projectId));
  const followDay = $derived(
    entry ? (app.followUps[`entry:${entry.id}`] ?? null) : tray ? (app.followUps[`tray:${tray.id}`] ?? null) : null,
  );
  const recurringId = $derived(entry ? entry.recurringId : (tray?.recurringId ?? null));
  const thing = $derived(recurringId ? (repeating.items.find((r) => r.id === recurringId) ?? null) : null);
  // A tray roll-up's project comes from its tickets, so it isn't picked here.
  const canPickProject = $derived(!entry?.isTray);
  const timing = $derived(!!entry && timer.running && timer.target?.entryId === entry.id);

  async function save() {
    saving = null;
    const text = draft;
    if (text.trim() === saved.trim()) {
      status = '';
      return;
    }
    status = 'saving';
    if (entry) {
      const updated = await attempt(() => api.setEntryNotes(entry.id, text));
      if (updated) putEntry(updated);
    } else if (tray) {
      const day = await attempt(() => api.traySetNote(tray.id, text));
      if (day && app.tray?.date === day.date) app.tray = day;
    }
    saved = text;
    status = 'saved';
    onsaved?.();
  }

  function typed() {
    if (saving) clearTimeout(saving);
    saving = setTimeout(() => void save(), SAVE_AFTER_MS);
  }

  function pickProject(anchor: HTMLElement) {
    const pick = (id: string | null) => {
      if (entry) void moveEntryTo(entry, id);
      else if (tray) void setTrayProject(tray, id);
    };
    const pickRepeating = (id: string) => {
      if (entry) void moveEntryToRepeating(entry, id);
      else if (tray) void setTrayRepeating(tray, id);
    };
    openMenu(anchor, {
      sections: [
        {
          head: tray ? 'Count its time under' : 'Project',
          choices: [
            ...app.projects
              .filter((p) => !p.archived || p.id === projectId)
              .map((p) => ({ label: p.name, color: p.color, checked: p.id === projectId, act: () => pick(p.id) })),
            ...repeatingChoices(recurringId, pickRepeating),
            {
              label: 'No project',
              color: null,
              checked: projectId === null && recurringId === null,
              act: () => pick(null),
            },
          ],
        },
      ],
    });
  }

  async function attach() {
    if (!entry || !ticketText.trim()) return;
    const updated = await attempt(() => api.addEntryTickets(entry.id, ticketText));
    if (!updated) return;
    ticketText = '';
    putEntry(updated);
    void refresh();
  }

  async function tickTicket(ticketId: string, done: boolean) {
    const updated = await attempt(() => api.ticketSetDone(ticketId, done));
    if (!updated) return;
    putEntry(updated);
    onsaved?.();
  }

  async function detach(ticketId: string, number: string) {
    const updated = await attempt(() => api.removeTicket(ticketId));
    if (!updated) return;
    putEntry(updated);
    toast(`Removed ${number}`, 'Undo', () => {
      void attempt(() => api.addEntryTickets(updated.id, number)).then(refresh);
    });
  }

  /** The notepad moves into a window; what's typed so far is saved first. */
  async function popOut() {
    if (saving) clearTimeout(saving);
    await save();
    if (entry) openWindow('entry', entry.id);
    else if (tray) openWindow('tray', tray.id);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="panel"
  aria-label="Notepad and options for {entry ? entry.description || 'this entry' : tray?.ticket}"
  onkeydown={onKey}
  onpointerdown={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <label class="lbl" for="pad-{entry?.id ?? tray?.id}">
    Notepad
    <span class="status" aria-live="polite">{status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved' : ''}</span>
    <button type="button" class="pop" title="Open the notepad in a window you can move around" onclick={popOut}>
      <Icon name="expand" size={12} /> Pop out
    </button>
  </label>
  <textarea
    id="pad-{entry?.id ?? tray?.id}"
    class="pad"
    bind:this={pad}
    bind:value={draft}
    rows="3"
    placeholder={tray ? 'What happened, what is left, who to chase…' : 'Notes about this work…'}
    oninput={typed}
    onblur={() => {
      if (saving) clearTimeout(saving);
      void save();
    }}
  ></textarea>

  {#if entry && !entry.isTray}
    <div class="row">
      <span class="lbl">Tickets</span>
      {#each entry.tickets as t (t.id)}
        <span class="etk">
          <button
            type="button"
            class="tick"
            class:done={t.done}
            aria-pressed={t.done}
            title={t.done ? 'Done. Click if it is still open.' : 'Still open. Click when it is done.'}
            aria-label={t.done ? `${t.number} is done; mark it still open` : `${t.number} is still open; mark it done`}
            onclick={() => void tickTicket(t.id, !t.done)}
          >
            {#if t.done}<Icon name="check" size={10} />{/if}
          </button>
          <TicketChip number={t.number} links={t.links} color={projectColor(entry.projectId)} onRemove={() => void detach(t.id, t.number)} />
        </span>
      {/each}
      <input
        class="attach"
        bind:value={ticketText}
        placeholder="+ Attach a ticket"
        aria-label="Attach a ticket to {entry.description || 'this entry'}"
        autocomplete="off"
        spellcheck="false"
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void attach();
          }
        }}
        onblur={() => void attach()}
      />
    </div>
  {/if}

  {#if canPickProject}
    <div class="row">
      <span class="lbl">{tray ? 'Time counts under' : 'Project'}</span>
      <button type="button" class="proj" onclick={(e) => pickProject(e.currentTarget as HTMLElement)}>
        <span class="sw" style:background={project ? projectColor(projectId) : 'transparent'}></span>
        {#if thing && !project}
          <Icon name="repeat" size={13} />
          {thing.name}
        {:else}
          {project?.name ?? (tray ? "The day's Tickets entry" : 'No project')}
        {/if}
        <Icon name="right" size={12} />
      </button>
    </div>
  {/if}

  <div class="acts">
    {#if tray}
      <button type="button" class="act" class:ok={tray.done} class:open={!tray.done} onclick={() => void setTrayDone(tray, !tray.done)}>
        <Icon name={tray.done ? 'check' : 'clock'} size={15} />
        {tray.done ? 'Done' : 'Still open'}
      </button>
    {/if}
    <button
      type="button"
      class="act"
      class:open={!!followDay}
      aria-haspopup="menu"
      onclick={(e) => {
        const section = entry
          ? bringUpSection('entry', entry.id, entry.description)
          : tray
            ? bringUpSection('tray', tray.id, tray.note ? `${tray.ticket}: ${tray.note}` : tray.ticket)
            : null;
        if (section) openMenu(e.currentTarget as HTMLElement, { sections: [section] });
      }}
    >
      <Icon name="sun" size={15} />
      {followDay ? `Follow up ${followWords(followDay, app.today)}` : 'Follow up…'}
    </button>
    {#if entry && !entry.isTray}
      <span class="time" role="group" aria-label="Add time">
        <button type="button" onclick={() => void addEntryTime(entry, 15)}>+15m</button>
        <button type="button" onclick={() => void addEntryTime(entry, 30)}>+30m</button>
        <button type="button" onclick={() => void addEntryTime(entry, 60)}>+1h</button>
      </span>
    {/if}
    {#if entry && canTime(entry)}
      <button type="button" class="act" class:ok={timing} onclick={() => void (timing ? stopTimer() : startTimer(entry))}>
        <Icon name="play" size={13} />
        {timing ? 'Stop the timer' : 'Start the timer'}
      </button>
    {/if}
    <button type="button" class="act" onclick={(e) => openMenu(e.currentTarget as HTMLElement, menu())}>
      <Icon name="more" size={15} /> More
    </button>
  </div>
</section>

<style>
  .etk {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .tick {
    width: 16px;
    height: 16px;
    padding: 0;
    flex: none;
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

  .panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 12px;
    background: var(--paper);
    border: 1px solid var(--rule);
    border-top: 0;
    border-radius: 0 0 10px 10px;
    box-shadow: var(--lift);
    cursor: default;
    user-select: text;
    transform-origin: top center;
    animation: unfold calc(0.24s * var(--m)) var(--spring);
  }

  @keyframes unfold {
    from {
      opacity: 0;
      transform: translateY(-8px) scaleY(0.92);
    }
  }

  .lbl {
    font: 700 10px var(--ui);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--mut);
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .status {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 500;
  }

  .pop {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 0;
    background: none;
    color: var(--accent);
    font: 600 11.5px var(--ui);
    text-transform: none;
    letter-spacing: 0;
    cursor: pointer;
    padding: 0;
  }

  .pad {
    width: 100%;
    min-height: 64px;
    resize: vertical;
    font: 13px/1.5 var(--ui);
    color: var(--ink);
    background: var(--desk);
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 7px 9px;
  }

  .pad:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: var(--paper);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .attach {
    font: 12px var(--ui);
    width: 130px;
    padding: 3px 8px;
    border: 1px dashed var(--rule);
    border-radius: 7px;
    background: transparent;
    color: var(--ink);
  }

  .attach:focus {
    outline: none;
    border-style: solid;
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .proj,
  .act,
  .time button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: 600 12.5px var(--ui);
    color: var(--ink);
    background: var(--paper);
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 5px 9px;
    cursor: pointer;
  }

  .proj:hover,
  .act:hover,
  .time button:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    border: 1px solid var(--rule);
  }

  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .act.ok {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 10%, var(--paper));
    border-color: color-mix(in srgb, var(--ok) 40%, transparent);
  }

  .act.open {
    color: var(--warn);
    background: var(--warn-wash);
    border-color: color-mix(in srgb, var(--warn) 40%, transparent);
  }

  .time {
    display: inline-flex;
    gap: 4px;
  }

  .time button {
    padding: 5px 7px;
    font-family: var(--mono);
    font-size: 12px;
  }

  @media (prefers-reduced-motion: reduce) {
    .panel {
      animation: none;
    }
  }
</style>
