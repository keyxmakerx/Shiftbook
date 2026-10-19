<script lang="ts">
  // One card on the Desk (the Canvas): an entry, a Done ticket, a note or an Inbox scrap.
  // Drag to move it (saved when you let go); double-click to open it; right-click for its
  // paper menu. The card is the record itself: changes show everywhere else too.
  import { api } from '../lib/api';
  import type { CanvasCard } from '../lib/api';
  import { entryMenu } from '../lib/entryActions';
  import { fmtMinutes, relativeDay } from '../lib/format';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { KIND_LABEL, snippet } from '../lib/notes';
  import { copyText } from '../lib/protection.svelte';
  import { app, attempt, focusLog, openNote, projectById, projectColor, refresh, refreshInbox } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  let {
    card,
    zoom,
    onmove,
    onchanged,
  }: {
    card: CanvasCard;
    zoom: number;
    /** Dropped at a new spot, in canvas units. */
    onmove: (x: number, y: number) => void;
    /** Something about the record changed; reload the Desk. */
    onchanged: () => void;
  } = $props();

  let el: HTMLElement | undefined = $state();
  let drag = $state<{ dx: number; dy: number } | null>(null);
  let editing = $state(false);
  let noteText = $state('');

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
        ? 'var(--accent)'
        : card.note
          ? card.note.projectId
            ? projectColor(card.note.projectId)
            : (NOTE_COLOR[card.note.kind] ?? 'var(--none)')
          : 'var(--bad)',
  );

  const dayWord = (iso: string) => (iso === app.today ? '' : ` · ${relativeDay(iso, app.today)}`);

  // ----- moving -----
  function down(e: PointerEvent) {
    if (e.button !== 0 || editing) return;
    if ((e.target as HTMLElement).closest('input, textarea, button')) return;
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
    };
    const up = () => {
      el?.removeEventListener('pointermove', move);
      el?.removeEventListener('pointerup', up);
      el?.removeEventListener('pointercancel', up);
      if (moved && drag) onmove(Math.round(card.x + drag.dx), Math.round(card.y + drag.dy));
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

  function open() {
    if (card.entry) app.drawer = card.entry.id;
    else if (card.note) openNote(card.note.id);
    else if (card.inbox) void logScrap();
    else if (card.tray) {
      noteText = card.tray.item.note ?? '';
      editing = true;
    }
  }

  async function saveTrayNote() {
    editing = false;
    if (!card.tray || noteText.trim() === (card.tray.item.note ?? '')) return;
    const day = await attempt(() => api.traySetNote(card.tray!.item.id, noteText));
    if (day) onchanged();
  }

  /** Focus a box as soon as it appears (after the double-click that opened it settles). */
  function focusNow(node: HTMLInputElement) {
    requestAnimationFrame(() => node.focus());
  }

  // ----- menus -----
  async function copy(text: string, what: string) {
    toast((await copyText(text)) ? `Copied ${what}` : "Couldn't copy here.");
  }

  function menu(): MenuSpec | null {
    if (card.entry) return entryMenu(card.entry);
    if (card.tray) {
      const { item } = card.tray;
      return {
        sections: [
          {
            head: `${item.ticket} · ${fmtMinutes(item.minutes)}`,
            items: [
              { label: 'Copy ticket number', icon: 'copy', side: true, act: () => void copy(item.ticket, item.ticket) },
              { label: item.note ? 'Change the few words' : 'Add a few words', icon: 'edit', side: true, act: open },
            ],
          },
          {
            items: [
              {
                label: 'Take out of the tray',
                icon: 'trash',
                danger: true,
                top: true,
                act: () =>
                  void attempt(() => api.trayRemove(item.id)).then((d) => {
                    if (!d) return;
                    void refresh();
                    toast(`Took ${item.ticket} out of the tray`, 'Undo', () => {
                      void attempt(() => api.trayRestore(item.id)).then(refresh);
                    });
                  }),
              },
            ],
          },
        ],
      };
    }
    if (card.note) {
      const note = card.note;
      return {
        sections: [
          {
            head: note.title,
            items: [
              { label: 'Open in Notes', icon: 'open', side: true, act: () => openNote(note.id) },
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
                    });
                  }),
              },
            ],
          },
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
                    });
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
  bind:this={el}
  style:--c={color}
  style:left="{card.x + (drag?.dx ?? 0)}px"
  style:top="{card.y + (drag?.dy ?? 0)}px"
  style:z-index={drag ? 99999 : card.z}
  role="button"
  tabindex="0"
  title="Drag to move · double-click to open · right-click for options"
  onpointerdown={down}
  ondblclick={(e) => {
    e.stopPropagation();
    open();
  }}
  onkeydown={(e) => {
    if (e.target === el && e.key === 'Enter') open();
  }}
  use:paperMenu={menu}
>
  {#if card.entry}
    {@const e = card.entry}
    <span class="kind">Entry · {e.hours} h{dayWord(e.date)}</span>
    <span class="title">{e.description || 'Untitled'}</span>
    <span class="sub">{projectById(e.projectId)?.name ?? 'No project'}</span>
    {#if e.tickets.length}
      <span class="tickets">{e.tickets.map((t) => t.number).join(' · ')}</span>
    {/if}
  {:else if card.tray}
    {@const t = card.tray}
    <span class="kind">Done ticket · {fmtMinutes(t.item.minutes)}{dayWord(t.date)}</span>
    <span class="title mono">{t.item.ticket}</span>
    {#if editing}
      <input
        class="text-in few"
        use:focusNow
        bind:value={noteText}
        maxlength="300"
        placeholder="A few words…"
        aria-label="A few words about {t.item.ticket}"
        onblur={saveTrayNote}
        onkeydown={(ev) => {
          if (ev.key === 'Enter') (ev.currentTarget as HTMLInputElement).blur();
          if (ev.key === 'Escape') {
            ev.stopPropagation();
            editing = false;
          }
        }}
      />
    {:else}
      <span class="sub" class:muted={!t.item.note}>{t.item.note ?? 'Double-click to add a few words'}</span>
    {/if}
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

<style>
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
    transition:
      box-shadow 0.2s,
      transform 0.25s var(--spring);
  }

  .card:hover {
    box-shadow: var(--lift);
  }

  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
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

  .tickets {
    font: 600 11px var(--mono);
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .few {
    font-size: 12.5px;
    padding: 4px 7px;
  }

  @media (prefers-reduced-motion: reduce) {
    .card,
    .card.lifted {
      transition: none;
      transform: none;
    }
  }
</style>
