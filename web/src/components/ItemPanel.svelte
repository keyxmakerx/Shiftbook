<script lang="ts">
  // The slide-out under an entry or a ticket (round 12): one click on a row on Day, a card on
  // the Desk or a ticket in the tray opens it right underneath. It holds the everyday things:
  // the notepad (saved as you type), the entry's tickets, the project, Bring up tomorrow, done
  // or still open (tickets), more time and, only when the timer is turned on, Start. The full
  // menu stays on right-click and on "More". Esc folds it away.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { Entry, TrayItem } from '../lib/api';
  import { bringUp } from '../lib/bringUp';
  import { addEntryTime, canTime, moveEntryTo } from '../lib/entryActions';
  import { openMenu, type MenuSpec } from '../lib/menu.svelte';
  import { app, attempt, projectById, projectColor, putEntry, refresh } from '../lib/store.svelte';
  import { setTrayDone, setTrayProject } from '../lib/trayActions';
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
    openMenu(anchor, {
      sections: [
        {
          head: tray ? 'Count its time under' : 'Project',
          choices: [
            ...app.projects
              .filter((p) => !p.archived || p.id === projectId)
              .map((p) => ({ label: p.name, color: p.color, checked: p.id === projectId, act: () => pick(p.id) })),
            { label: 'No project', color: null, checked: projectId === null, act: () => pick(null) },
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

  async function detach(ticketId: string, number: string) {
    const updated = await attempt(() => api.removeTicket(ticketId));
    if (!updated) return;
    putEntry(updated);
    toast(`Removed ${number}`, 'Undo', () => {
      void attempt(() => api.addEntryTickets(updated.id, number)).then(refresh);
    });
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
        <TicketChip number={t.number} links={t.links} color={projectColor(entry.projectId)} onRemove={() => void detach(t.id, t.number)} />
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
        {project?.name ?? (tray ? "The day's Tickets entry" : 'No project')}
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
      onclick={() => {
        if (entry) void bringUp('entry', entry.id, entry.description);
        else if (tray) void bringUp('tray', tray.id, tray.note ? `${tray.ticket}: ${tray.note}` : tray.ticket);
      }}
    >
      <Icon name="sun" size={15} /> Bring up tomorrow
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
