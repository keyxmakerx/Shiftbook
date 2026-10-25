<script lang="ts">
  // One card on the Desk (the Canvas): an entry, a ticket, a note or an Inbox scrap.
  // Drag to move it (saved when you let go); click an entry or a ticket for its notepad and
  // options underneath (round 12); double-click to open it; right-click for its paper menu.
  // The card is the record itself: changes show everywhere else too.
  import { api } from '../lib/api';
  import type { CanvasCard } from '../lib/api';
  import { bringUp, bringUpSection } from '../lib/bringUp';
  import { entryMenu } from '../lib/entryActions';
  import { fmtMinutes, relativeDay } from '../lib/format';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { KIND_LABEL, snippet } from '../lib/notes';
  import { closePanel, openNotepad, panel } from '../lib/panel.svelte';
  import { app, attempt, focusLog, openNote, projectById, projectColor, refreshInbox } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { logNoteSection } from '../lib/noteActions';
  import { pinMenuItem } from '../lib/pins.svelte';
  import { trayMenu } from '../lib/trayActions';
  import { openWindow } from '../lib/windows.svelte';
  import Icon from './Icon.svelte';
  import ItemPanel from './ItemPanel.svelte';
  import TicketGroup from './TicketGroup.svelte';
  import { workTickets } from '../lib/workTickets';

  let {
    card,
    x = card.x,
    y = card.y,
    zoom,
    locked = false,
    linking = false,
    selected = false,
    dropping = false,
    faded = false,
    extra,
    ondrag,
    onmove,
    onchanged,
    onlink,
    onpick,
    onopen,
  }: {
    card: CanvasCard;
    zoom: number;
    /** Dropped at a new spot, in canvas units, with the pointer's screen point. */
    onmove: (x: number, y: number, at: { cx: number; cy: number }) => void;
    /** Something about the record changed; reload the Desk. */
    onchanged: () => void;
    /** Shown where (the timeline puts cards elsewhere); defaults to the card's own spot. */
    x?: number;
    y?: number;
    /** Can't be dragged (the timeline). */
    locked?: boolean;
    /** Picking the other end of a new link: a click picks this card. */
    linking?: boolean;
    /** Part of the selection (they move together). */
    selected?: boolean;
    /** "Just today" is on and this card isn't from today: it fades back. */
    faded?: boolean;
    /** Something is being dragged onto it (it would make a pile). */
    dropping?: boolean;
    /** More menu sections from the Desk (filing, piles). */
    extra?: () => MenuSpec['sections'];
    /** Being dragged this far (canvas units), with the pointer's screen point. */
    ondrag?: (dx: number, dy: number, at: { cx: number; cy: number }) => void;
    /** "Link to another card" was chosen on this card. */
    onlink?: () => void;
    /** This card was picked as the other end of a link. */
    onpick?: () => void;
    /** Opening it (double-click or Enter) does this instead (a board opens notes in its Notebook). */
    onopen?: () => void;
  } = $props();

  let el: HTMLElement | undefined = $state();
  /** The entry's tickets are open in their floating box (round 14). */
  let group = $state(false);
  let drag = $state<{ dx: number; dy: number } | null>(null);
  let height = $state(0);

  const key = $derived(`${card.kind}:${card.id}`);
  const open = $derived(panel.open === key);
  // Entries and tickets have the slide-out; notes open in Notes, scraps get logged.
  const hasPanel = $derived(!!card.entry || !!card.tray);
  let openedAt = 0;

  /** A click opens the slide-out; a later click folds it (a double-click's second click doesn't). */
  function clicked() {
    if (!hasPanel) return;
    if (!open) {
      panel.focusPad = false;
      panel.open = key;
      openedAt = Date.now();
    } else if (Date.now() - openedAt > 400) {
      closePanel();
    }
  }

  const NOTE_COLOR: Record<string, string> = {
    jot: '#de9930',
    daily: '#6b7682',
    meeting: '#c98a22',
    page: '#8c4f7a',
    mop: '#178f7c',
  };

  const color = $derived(
    card.entry
      ? projectColor(card.entry.projectId)
      : card.tray
        ? card.tray.item.projectId
          ? projectColor(card.tray.item.projectId)
          : 'var(--accent)'
        : card.note
          ? card.note.projectId
            ? projectColor(card.note.projectId)
            : (NOTE_COLOR[card.note.kind] ?? 'var(--none)')
          : 'var(--bad)',
  );

  const dayWord = (iso: string) => (iso === app.today ? '' : ` · ${relativeDay(iso, app.today)}`);

  // ----- moving -----
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    if (linking) {
      e.stopPropagation();
      onpick?.();
      return;
    }
    // A button on the card (the tickets strip) gets its own click; the Desk mustn't take the
    // pointer for panning.
    if ((e.target as HTMLElement).closest('input, textarea, button')) {
      e.stopPropagation();
      return;
    }
    // No text selection or browser drag from a card: either one takes the pointer away and the
    // card sticks. Focus by hand, since this stops the press from focusing it.
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    el?.focus({ preventScroll: true });
    if (locked) {
      clicked();
      return;
    }
    e.stopPropagation();
    const start = { x: e.clientX, y: e.clientY };
    let moved = false;
    el?.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      const dx = (m.clientX - start.x) / zoom;
      const dy = (m.clientY - start.y) / zoom;
      if (!moved && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 4) return;
      moved = true;
      drag = { dx, dy };
      ondrag?.(dx, dy, { cx: m.clientX, cy: m.clientY });
    };
    const up = (u: PointerEvent) => {
      el?.removeEventListener('pointermove', move);
      el?.removeEventListener('pointerup', up);
      el?.removeEventListener('pointercancel', up);
      if (moved && drag) onmove(Math.round(x + drag.dx), Math.round(y + drag.dy), { cx: u.clientX, cy: u.clientY });
      else clicked();
      drag = null;
    };
    el?.addEventListener('pointermove', move);
    el?.addEventListener('pointerup', up);
    el?.addEventListener('pointercancel', up);
  }

  // ----- opening -----
  async function logScrap() {
    if (!card.inbox) return;
    const scrap = card.inbox;
    if (await attempt(() => api.setInboxDeleted(scrap.id, true).then(() => true))) {
      await attempt(refreshInbox);
      focusLog(scrap.text);
    }
  }

  function openCard() {
    if (onopen) onopen();
    else if (card.entry) {
      closePanel();
      app.drawer = card.entry.id;
    } else if (card.note) openNote(card.note.id);
    else if (card.inbox) void logScrap();
    else if (card.tray) openNotepad(key);
  }

  // ----- menus -----
  /** The card's own menu, plus the Desk's options for it. */
  function menu(): MenuSpec | null {
    const own = cardMenu();
    if (!own) return null;
    if (onlink) {
      own.sections.splice(1, 0, {
        items: [{ label: 'Link to another card', icon: 'link', top: true, act: () => onlink?.() }],
      });
    }
    if (extra) own.sections.splice(Math.max(1, own.sections.length - 1), 0, ...extra());
    return own;
  }

  function cardMenu(): MenuSpec | null {
    if (card.entry) return entryMenu(card.entry);
    if (card.tray) return trayMenu(card.tray.item, () => openNotepad(key));
    if (card.note) {
      const note = card.note;
      return {
        sections: [
          {
            head: note.title,
            items: [
              { label: 'Open in Notes', icon: 'open', side: true, act: () => openNote(note.id) },
              { label: 'Open in a window', icon: 'expand', side: true, act: () => openWindow('note', note.id) },
              { label: 'Bring up tomorrow', icon: 'sun', side: true, act: () => void bringUp('note', note.id, note.title) },
              pinMenuItem('note', note.id),
              {
                label: 'Archive',
                icon: 'archive',
                side: true,
                act: () =>
                  void attempt(() => api.setNoteArchived(note.id, true).then(() => true)).then((ok) => {
                    if (!ok) return;
                    onchanged();
                    toast(`Archived “${note.title}”`, 'Undo', () => {
                      void attempt(() => api.setNoteArchived(note.id, false)).then(onchanged);
                    }, () => void attempt(() => api.setNoteArchived(note.id, true)).then(onchanged));
                  }),
              },
            ],
          },
          bringUpSection('note', note.id, note.title),
          ...(note.kind === 'jot' ? [logNoteSection(note, onchanged)] : []),
        ],
      };
    }
    if (card.inbox) {
      const scrap = card.inbox;
      return {
        sections: [
          {
            head: 'Inbox scrap',
            items: [
              { label: 'Log it', icon: 'log', side: true, act: () => void logScrap() },
              {
                label: 'Remove',
                icon: 'trash',
                danger: true,
                top: true,
                act: () =>
                  void attempt(() => api.setInboxDeleted(scrap.id, true).then(() => true)).then((ok) => {
                    if (!ok) return;
                    void attempt(refreshInbox);
                    toast('Removed from the Inbox', 'Undo', () => {
                      void attempt(() => api.setInboxDeleted(scrap.id, false)).then(() => attempt(refreshInbox));
                    }, () => void attempt(() => api.setInboxDeleted(scrap.id, true)).then(() => attempt(refreshInbox)));
                  }),
              },
            ],
          },
        ],
      };
    }
    return null;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="card paper"
  class:scrap={!!card.inbox}
  class:lifted={!!drag}
  class:locked
  class:picking={linking}
  class:selected
  class:dropping
  class:faded
  class:opened={open}
  bind:this={el}
  bind:offsetHeight={height}
  style:--c={color}
  style:left="{x + (drag?.dx ?? 0)}px"
  style:top="{y + (drag?.dy ?? 0)}px"
  style:z-index={drag ? 99999 : open ? 99998 : card.z}
  role="button"
  tabindex="0"
  aria-expanded={hasPanel ? open : undefined}
  title={hasPanel
    ? 'Drag to move · click for the notepad and options · double-click to open · right-click for everything'
    : 'Drag to move · double-click to open · right-click for options'}
  onpointerdown={down}
  ondblclick={(e) => {
    e.stopPropagation();
    openCard();
  }}
  onkeydown={(e) => {
    if (e.target !== el) return;
    if (e.key === 'Enter') openCard();
    else if (e.key === ' ' && hasPanel) {
      e.preventDefault();
      clicked();
    }
  }}
  use:paperMenu={menu}
>
  {#if card.entry}
    {@const e = card.entry}
    <span class="kind">
      {#if card.work.length}
        {@const days = new Set([e.date, ...card.work.map((w) => w.date)]).size}
        Entry · {card.workHours} h · {days > 1 ? `${days} days` : `${card.work.length + 1} entries`}
      {:else}Entry · {e.hours} h{dayWord(e.date)}{/if}
    </span>
    <span class="title">{e.description || 'Untitled'}</span>
    <span class="sub">{projectById(e.projectId)?.name ?? 'No project'}</span>
    {@const all = workTickets(card)}
    {#if all.length}
      {@const still = all.filter((t) => !t.done).length}
      <button
        type="button"
        class="tstrip"
        class:still={still > 0}
        aria-expanded={group}
        title="{all.map((t) => t.number).join(', ')} · click to open them"
        onclick={(ev) => {
          ev.stopPropagation();
          closePanel();
          group = !group;
        }}
      >
        <Icon name="ticket" size={12} />
        <span class="tn">{all.length === 1 ? all[0]!.number : `${all.length} tickets`}</span>
        {#if still}<span class="so">{still} open</span>{/if}
      </button>
    {/if}
    {#if e.notes}<span class="padline">✎ {e.notes.split('\n')[0]}</span>{/if}
  {:else if card.tray}
    {@const t = card.tray}
    <span class="kind">
      {#if t.item.done}Ticket{:else}<span class="open">Still open</span>{/if} · {fmtMinutes(t.item.minutes)}{dayWord(t.date)}
    </span>
    <span class="title mono">{t.item.ticket}</span>
    <span class="sub" class:muted={!t.item.note}>
      {t.item.note?.split('\n')[0] ?? (t.item.projectId ? projectById(t.item.projectId)?.name : 'Click for its notepad')}
    </span>
  {:else if card.note}
    {@const n = card.note}
    <span class="kind">{KIND_LABEL[n.kind]} note</span>
    <span class="title">{n.title}</span>
    {#if snippet(n.body, n.title, '', 90)}<span class="sub">{snippet(n.body, n.title, '', 90)}</span>{/if}
  {:else if card.inbox}
    <span class="kind">Inbox scrap</span>
    <span class="title">{card.inbox.text}</span>
  {/if}
</div>
{#if group && card.entry}
  <div class="tgroup" style:left="{x + 230}px" style:top="{y}px">
    <TicketGroup {card} {onchanged} onclose={() => (group = false)} />
  </div>
{/if}
{#if open && (card.entry || card.tray)}
  <div class="under" style:left="{x}px" style:top="{y + height}px">
    <ItemPanel
      entry={card.entry}
      tray={card.tray?.item}
      menu={() => menu() ?? { sections: [] }}
      focusPad={panel.focusPad}
      onclose={closePanel}
      onsaved={onchanged}
    />
  </div>
{/if}

<style>
  .card.faded:not(:hover, :focus-within, .lifted, .selected) {
    opacity: 0.3;
    filter: saturate(0.4);
  }

  .card {
    position: absolute;
    width: 220px;
    max-height: 120px;
    overflow: hidden;
    padding: 8px 11px 10px;
    border-top: 3px solid var(--c);
    border-radius: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    cursor: grab;
    user-select: none;
    touch-action: none;
    /* Cards glide to and from the timeline. */
    transition:
      box-shadow 0.2s,
      transform 0.25s var(--spring),
      left calc(0.55s * var(--m)) var(--ease),
      top calc(0.55s * var(--m)) var(--ease);
  }

  .card:hover {
    box-shadow: var(--lift);
  }

  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .card.locked {
    cursor: default;
  }

  .card.picking {
    cursor: crosshair;
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent),
      var(--shadow);
  }

  .card.lifted {
    cursor: grabbing;
    transform: scale(1.03) rotate(0.4deg);
    box-shadow: var(--lift);
    transition: none;
  }

  .card.scrap {
    outline: 1.5px dashed var(--bad);
    outline-offset: -1.5px;
  }

  .kind {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    color: var(--mut);
  }

  .title {
    font-weight: 600;
    font-size: 13.5px;
    line-height: 1.3;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  .sub {
    font-size: 12px;
    color: var(--ink-2);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .sub.muted {
    color: var(--mut);
    font-style: italic;
  }

  .card.dropping {
    box-shadow:
      0 0 0 3px var(--accent),
      var(--lift);
    transform: scale(1.02);
  }

  .card.selected {
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--accent) 70%, transparent),
      var(--shadow);
  }

  .card.opened {
    box-shadow:
      0 0 0 2px var(--accent),
      var(--lift);
    border-radius: 5px 5px 0 0;
  }

  .under {
    position: absolute;
    width: 300px;
    z-index: 99997;
  }

  .tgroup {
    position: absolute;
    z-index: 99997;
  }

  .tstrip {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    margin-top: 2px;
    padding: 2px 8px 2px 6px;
    border: 1px solid var(--rule);
    border-radius: 10px;
    background: color-mix(in srgb, var(--ink) 4%, transparent);
    font: 600 11px var(--mono);
    color: var(--ink-2);
    cursor: pointer;
  }

  .tstrip:hover,
  .tstrip[aria-expanded='true'] {
    border-color: var(--accent);
  }

  .tn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .so {
    font: 700 10px var(--ui);
    color: var(--warn);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .padline {
    font-size: 11.5px;
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .open {
    color: var(--warn);
  }

  @media (prefers-reduced-motion: reduce) {
    .card,
    .card.lifted {
      transition: none;
      transform: none;
    }
  }
</style>
