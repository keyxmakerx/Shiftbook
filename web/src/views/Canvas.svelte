<script module lang="ts">
  import { HOME, type CanvasView } from '../lib/canvasView';

  // Where you left the view, the panel and the links switch, kept while the app is open.
  let lastView: CanvasView = HOME;
  let panelOpen = true;
  let linksShown = true;
</script>

<script lang="ts">
  // The Desk: the Canvas where the day happens (round 9). Your entries, Done tickets, notes
  // and Inbox scraps as cards you arrange; only where they sit is new. The panel holds what
  // Day has for logging: today's hours, "Where was I?", Log work, Done tickets and the wrap-up.
  // Step 2 adds project areas (drop a card in to give it the project), links between cards
  // (automatic and drawn) and the timeline. The full plan is docs/CANVAS.md.
  import { onMount, tick, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { CanvasArea, CanvasCard as Card, CanvasLink, CardKind, CardRef } from '../lib/api';
  import { areaAt, CARD_H, CARD_W, fitAll, NEW_AREA, toCanvas, zoomAt } from '../lib/canvasView';
  import { moveEntryTo } from '../lib/entryActions';
  import { dayMonth, fmtHours, weekdayShort } from '../lib/format';
  import { openMenu, type MenuSpec } from '../lib/menu.svelte';
  import { app, attempt, projectById, refreshProjects, ribbonDay, ribbonWeekOf, selectDay } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import CanvasAreaBox from '../components/CanvasArea.svelte';
  import CanvasCard from '../components/CanvasCard.svelte';
  import Icon from '../components/Icon.svelte';
  import LogWork from '../components/LogWork.svelte';
  import TrayCard from '../components/TrayCard.svelte';
  import WhereWasI from '../components/WhereWasI.svelte';
  import WrapUp from '../components/WrapUp.svelte';

  let cards = $state<Card[]>([]);
  let areas = $state<CanvasArea[]>([]);
  let links = $state<CanvasLink[]>([]);
  let axis = $state<{ date: string; x: number }[]>([]);
  let mode = $state<'free' | 'timeline'>('free');
  let viewBeforeTimeline: CanvasView | null = null;
  let showLinks = $state(linksShown);
  let linkFrom = $state<Card | null>(null);
  let areaDrag = $state<{ index: number; dx: number; dy: number; members: Set<string> } | null>(null);
  let loaded = $state(false);
  let view = $state<CanvasView>(lastView);
  let panel = $state(panelOpen);
  let board: HTMLElement | undefined = $state();
  let menuAnchor: HTMLElement | undefined = $state();
  let anchorAt = $state({ x: 0, y: 0 });
  let jot = $state<{ x: number; y: number; text: string } | null>(null);
  let panning = $state(false);

  $effect(() => {
    lastView = view;
  });
  $effect(() => {
    panelOpen = panel;
  });
  $effect(() => {
    linksShown = showLinks;
  });

  // N (or anything that asks for Log work) opens the panel if it was folded away.
  $effect(() => {
    if (app.logFocus.serial > 0) untrack(() => (panel = true));
  });

  const today = $derived(ribbonDay(app.today));
  const week = $derived(ribbonWeekOf(app.today));

  async function load() {
    const desk = await attempt(() => api.canvas(app.today));
    if (desk) {
      cards = desk.cards;
      areas = desk.areas;
      links = desk.links;
      axis = desk.axis;
    }
    loaded = true;
  }

  const keyOf = (c: { kind: CardKind; id: string }) => `${c.kind}:${c.id}`;
  const refOf = (c: Card): CardRef => ({ kind: c.kind, id: c.id });

  /** Where a card shows: its own spot, its timeline spot, and moved along with an area being dragged. */
  function pos(c: Card): { x: number; y: number } {
    if (mode === 'timeline') return { x: c.timelineX, y: c.timelineY };
    if (areaDrag?.members.has(keyOf(c))) return { x: c.x + areaDrag.dx, y: c.y + areaDrag.dy };
    return { x: c.x, y: c.y };
  }

  function titleOf(c: Card): string {
    return c.entry?.description || c.tray?.item.ticket || c.note?.title || c.inbox?.text || 'this card';
  }

  // The Desk works on today; reload whenever the day's data changes anywhere.
  onMount(() => {
    if (app.selected !== app.today) void selectDay(app.today);
  });
  $effect(() => {
    void app.entries;
    void app.tray;
    void app.inbox;
    void app.ribbon;
    untrack(() => void load());
  });

  // ----- moving cards -----
  async function move(card: Card, x: number, y: number) {
    const top = Math.max(0, ...cards.map((c) => c.z)) + 1;
    const before = { x: card.x, y: card.y };
    cards = cards.map((c) => (c === card ? { ...c, x, y, z: top, placed: true } : c));
    await attempt(() => api.canvasMove(card.kind, card.id, x, y));
    // Into a project's area gives the card that project; out of its own area takes it away.
    const to = areaAt(areas, x, y);
    const from = areaAt(areas, before.x, before.y);
    const toProject = to >= 0 ? areas[to]!.projectId : null;
    const fromProject = from >= 0 ? areas[from]!.projectId : null;
    if (toProject === fromProject) return;
    const current = card.entry ? card.entry.projectId : card.note ? card.note.projectId : undefined;
    if (current === undefined) return;
    if (toProject && toProject !== current) await setProject(card, toProject);
    else if (!toProject && fromProject && current === fromProject) await setProject(card, null);
  }

  async function setProject(card: Card, projectId: string | null) {
    if (card.entry) {
      await moveEntryTo(card.entry, projectId);
      return;
    }
    const note = card.note;
    if (!note) return;
    const write = (pid: string | null) =>
      api.updateNote(note.id, { kind: note.kind, title: note.title, body: note.body, projectId: pid, tags: note.tags.join(', ') });
    if (!(await attempt(() => write(projectId)))) return;
    await load();
    toast(`Moved to ${projectById(projectId)?.name ?? 'No project'}`, 'Undo', () => {
      void attempt(() => write(note.projectId)).then(load);
    });
  }

  // ----- project areas -----
  function areaMove(index: number, dx: number, dy: number, done: boolean) {
    if (!areaDrag) {
      const members = new Set(cards.filter((c) => areaAt(areas, c.x, c.y) === index).map(keyOf));
      areaDrag = { index, dx, dy, members };
    } else areaDrag = { ...areaDrag, dx, dy };
    if (!done) return;
    const drag = areaDrag;
    areaDrag = null;
    const a = areas[index]!;
    if (drag.dx === 0 && drag.dy === 0) return;
    const moved = { ...a, x: a.x + drag.dx, y: a.y + drag.dy };
    areas = areas.map((x, i) => (i === index ? moved : x));
    cards = cards.map((c) => (drag.members.has(keyOf(c)) ? { ...c, x: c.x + drag.dx, y: c.y + drag.dy, placed: true } : c));
    void (async () => {
      await attempt(() => api.canvasArea(moved.projectId, moved.x, moved.y, moved.w, moved.h));
      for (const c of cards.filter((c) => drag.members.has(keyOf(c)))) {
        await attempt(() => api.canvasMove(c.kind, c.id, c.x, c.y));
      }
    })();
  }

  async function areaResize(index: number, w: number, h: number) {
    const a = areas[index]!;
    areas = areas.map((x, i) => (i === index ? { ...a, w, h } : x));
    await attempt(() => api.canvasArea(a.projectId, a.x, a.y, w, h));
  }

  async function addArea(projectId: string, x: number, y: number) {
    if (!(await attempt(() => api.canvasArea(projectId, x, y, NEW_AREA.w, NEW_AREA.h).then(() => true)))) return;
    await load();
    toast(`Drop cards into ${projectById(projectId)?.name ?? 'the area'} to give them that project.`);
  }

  function areaMenu(a: CanvasArea): MenuSpec {
    const p = projectById(a.projectId);
    return {
      sections: [
        {
          head: `${p?.name ?? 'Project'} area`,
          items: [
            {
              label: 'Take the area off the Desk',
              icon: 'trash',
              danger: true,
              side: true,
              act: () =>
                void attempt(() => api.canvasAreaRemove(a.projectId).then(() => true)).then(async (ok) => {
                  if (!ok) return;
                  await load();
                  toast('Area taken off the Desk. The project and its cards stay.', 'Undo', () => {
                    void attempt(() => api.canvasArea(a.projectId, a.x, a.y, a.w, a.h)).then(load);
                  });
                }),
            },
          ],
        },
        ...(p
          ? [
              {
                head: `Colour for ${p.name}`,
                swatches: {
                  current: p.color,
                  pick: (c: string) =>
                    void attempt(() =>
                      api.updateProject(p.id, { name: p.name, color: c, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: p.archived }),
                    ).then(refreshProjects),
                },
              },
            ]
          : []),
      ],
    };
  }

  // ----- links -----
  const centres = $derived(
    new Map(cards.map((c) => {
      const p = pos(c);
      return [keyOf(c), { x: p.x + CARD_W / 2, y: p.y + CARD_H / 2 }] as const;
    })),
  );

  function linkPath(l: CanvasLink): string | null {
    const a = centres.get(keyOf(l.a));
    const b = centres.get(keyOf(l.b));
    if (!a || !b) return null;
    // A gentle curve, bowing to one side so lines that cross stay readable.
    const mx = (a.x + b.x) / 2 - (b.y - a.y) * 0.12;
    const my = (a.y + b.y) / 2 + (b.x - a.x) * 0.12;
    return `M ${a.x} ${a.y} Q ${mx} ${my} ${b.x} ${b.y}`;
  }

  function why(l: CanvasLink): string {
    if (l.drawn || !l.reason) return 'Linked by you';
    return l.reason.kind === 'ticket' ? `Both mention ${l.reason.text}` : `Both say “${l.reason.text}”`;
  }

  async function linkMenu(l: CanvasLink, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const { sx, sy } = boardPoint(e);
    anchorAt = { x: sx, y: sy };
    await tick();
    if (!menuAnchor) return;
    openMenu(menuAnchor, {
      sections: [
        {
          head: why(l),
          items: [
            {
              label: 'Remove this link',
              icon: 'x',
              danger: true,
              side: true,
              act: () =>
                void attempt(() => (l.drawn ? api.canvasLinkReset(l.a, l.b) : api.canvasLink(l.a, l.b, false)).then(() => true)).then(
                  async (ok) => {
                    if (!ok) return;
                    await load();
                    toast('Link removed', 'Undo', () => {
                      void attempt(() => (l.drawn ? api.canvasLink(l.a, l.b, true) : api.canvasLinkReset(l.a, l.b))).then(load);
                    });
                  },
                ),
            },
          ],
        },
      ],
    });
  }

  async function pickLinkEnd(card: Card) {
    const from = linkFrom;
    linkFrom = null;
    if (!from || keyOf(from) === keyOf(card)) return;
    if (!(await attempt(() => api.canvasLink(refOf(from), refOf(card), true).then(() => true)))) return;
    showLinks = true;
    await load();
    toast(`Linked “${titleOf(from)}” and “${titleOf(card)}”`, 'Undo', () => {
      void attempt(() => api.canvasLinkReset(refOf(from), refOf(card))).then(load);
    });
  }

  $effect(() => {
    if (!linkFrom) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        linkFrom = null;
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });

  // ----- timeline -----
  async function toggleTimeline() {
    if (mode === 'free') {
      viewBeforeTimeline = view;
      mode = 'timeline';
      await tick();
      if (board) {
        view = fitAll(
          cards.map((c) => ({ x: c.timelineX, y: c.timelineY })),
          board.clientWidth,
          board.clientHeight,
        );
      }
    } else {
      mode = 'free';
      if (viewBeforeTimeline) view = viewBeforeTimeline;
    }
  }

  // ----- pan and zoom -----
  function boardPoint(e: { clientX: number; clientY: number }) {
    const r = board!.getBoundingClientRect();
    return { sx: e.clientX - r.left, sy: e.clientY - r.top };
  }

  function panStart(e: PointerEvent) {
    if (e.button !== 0 || !board) return;
    if ((e.target as HTMLElement).closest('.tools, .jot')) return;
    const startView = view;
    const start = { x: e.clientX, y: e.clientY };
    panning = true;
    board.setPointerCapture(e.pointerId);
    const moveFn = (m: PointerEvent) => {
      view = { ...startView, x: startView.x + m.clientX - start.x, y: startView.y + m.clientY - start.y };
    };
    const up = () => {
      panning = false;
      board?.removeEventListener('pointermove', moveFn);
      board?.removeEventListener('pointerup', up);
      board?.removeEventListener('pointercancel', up);
    };
    board.addEventListener('pointermove', moveFn);
    board.addEventListener('pointerup', up);
    board.addEventListener('pointercancel', up);
  }

  // Wheel pans; Ctrl + wheel (or a pinch) zooms around the pointer.
  $effect(() => {
    if (!board) return;
    const el = board;
    const onWheel = (e: WheelEvent) => {
      if ((e.target as HTMLElement).closest('textarea, input')) return;
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        const { sx, sy } = boardPoint(e);
        view = zoomAt(view, sx, sy, Math.exp(-e.deltaY * 0.0015));
      } else {
        view = { ...view, x: view.x - e.deltaX, y: view.y - e.deltaY };
      }
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  });

  function zoomBy(factor: number) {
    if (!board) return;
    view = zoomAt(view, board.clientWidth / 2, board.clientHeight / 2, factor);
  }

  function fit() {
    if (!board) return;
    const spots = [...cards.map(pos), ...(mode === 'free' ? areas.flatMap((a) => [{ x: a.x, y: a.y }, { x: a.x + a.w - CARD_W, y: a.y + a.h - CARD_H }]) : [])];
    view = fitAll(spots, board.clientWidth, board.clientHeight);
  }

  // ----- jots on empty paper -----
  function startJot(sx: number, sy: number) {
    const at = toCanvas(view, sx, sy);
    jot = { x: Math.round(at.x), y: Math.round(at.y), text: '' };
  }

  async function saveJot() {
    const draft = jot;
    jot = null;
    if (!draft || !draft.text.trim()) return;
    const note = await attempt(() => api.createNote({ kind: 'jot', title: '', body: draft.text, projectId: null, tags: '' }));
    if (!note) return;
    await attempt(() => api.canvasMove('note' as CardKind, note.id, draft.x, draft.y));
    await load();
    toast('Jotted down. It’s in Notes too.');
  }

  async function emptyMenu(e: MouseEvent) {
    e.preventDefault();
    const { sx, sy } = boardPoint(e);
    anchorAt = { x: sx, y: sy };
    await tick();
    if (!menuAnchor) return;
    const at = toCanvas(view, sx, sy);
    const without = app.projects.filter((p) => !p.archived && !areas.some((a) => a.projectId === p.id));
    const sections: MenuSpec['sections'] = [
      {
        head: 'The Desk',
        items: [
          { label: 'New jot here', icon: 'note', side: true, act: () => startJot(sx, sy) },
          { label: 'Fit everything', icon: 'open', side: true, act: fit },
          { label: mode === 'free' ? 'Lay out as a timeline' : 'Back to my layout', icon: 'calendar', top: true, act: () => void toggleTimeline() },
          { label: showLinks ? 'Hide links' : 'Show links', icon: 'link', top: true, act: () => (showLinks = !showLinks) },
        ],
      },
    ];
    if (mode === 'free' && without.length > 0) {
      sections.push({
        head: 'Project area here',
        choices: without.map((p) => ({ label: p.name, color: p.color, checked: false, act: () => void addArea(p.id, Math.round(at.x), Math.round(at.y)) })),
      });
    }
    openMenu(menuAnchor, { sections });
  }
</script>

<div class="desk" class:closed={!panel}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="board"
    class:panning
    bind:this={board}
    onpointerdown={panStart}
    ondblclick={(e) => {
      if ((e.target as HTMLElement).closest('.tools')) return;
      const { sx, sy } = boardPoint(e);
      startJot(sx, sy);
    }}
    oncontextmenu={emptyMenu}
    onscroll={(e) => {
      // Focus moving to a card half off-screen would scroll the board; panning is ours.
      const el = e.currentTarget as HTMLElement;
      el.scrollLeft = 0;
      el.scrollTop = 0;
    }}
  >
    <div class="world" class:moving={!!areaDrag} style:transform="translate({view.x}px, {view.y}px) scale({view.zoom})">
      {#if mode === 'free'}
        {#each areas as area, i (area.projectId)}
          <CanvasAreaBox
            area={areaDrag?.index === i ? { ...area, x: area.x + areaDrag.dx, y: area.y + areaDrag.dy } : area}
            zoom={view.zoom}
            onmove={(dx, dy, done) => areaMove(i, dx, dy, done)}
            onresize={(w, h) => void areaResize(i, w, h)}
            menu={() => areaMenu(area)}
          />
        {/each}
      {:else}
        {#each axis as day (day.date)}
          <span class="day" style:left="{day.x}px">{weekdayShort(day.date)} {dayMonth(day.date)}</span>
        {/each}
      {/if}
      {#if showLinks}
        <svg class="links" aria-hidden="true">
          {#each links as l (keyOf(l.a) + '|' + keyOf(l.b))}
            {@const d = linkPath(l)}
            {#if d}
              <path class="line" class:drawn={l.drawn} class:ticket={l.reason?.kind === 'ticket'} {d} />
              <!-- Lines take a click or right-click; the Links button hides them all. -->
              <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
              <path class="hit" {d} oncontextmenu={(e) => void linkMenu(l, e)} onclick={(e) => void linkMenu(l, e)}><title>{why(l)}</title></path>
            {/if}
          {/each}
        </svg>
      {/if}
      {#each cards as card (card.kind + card.id)}
        {@const p = pos(card)}
        <CanvasCard
          {card}
          x={p.x}
          y={p.y}
          zoom={view.zoom}
          locked={mode === 'timeline'}
          linking={!!linkFrom}
          onmove={(x, y) => void move(card, x, y)}
          onchanged={() => void load()}
          onlink={() => (linkFrom = card)}
          onpick={() => void pickLinkEnd(card)}
        />
      {/each}
      {#if jot}
        <!-- svelte-ignore a11y_autofocus -->
        <textarea
          class="text-in jot"
          style:left="{jot.x}px"
          style:top="{jot.y}px"
          bind:value={jot.text}
          rows="3"
          placeholder="Jot something. Enter keeps it, Esc lets it go."
          aria-label="New jot"
          autofocus
          onpointerdown={(e) => e.stopPropagation()}
          ondblclick={(e) => e.stopPropagation()}
          onblur={saveJot}
          onkeydown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              (e.currentTarget as HTMLTextAreaElement).blur();
            } else if (e.key === 'Escape') {
              e.stopPropagation();
              jot = null;
            }
          }}
        ></textarea>
      {/if}
    </div>
    <span class="anchor" bind:this={menuAnchor} style:left="{anchorAt.x}px" style:top="{anchorAt.y}px"></span>

    {#if loaded && cards.length === 0 && !jot}
      <div class="empty">
        <p class="big">Your Desk is clear.</p>
        <p>Log work or paste finished tickets on the right and they appear here as cards. Double-click anywhere to jot something down.</p>
      </div>
    {/if}

    <div class="tools" role="toolbar" aria-label="Desk view">
      <button type="button" aria-label="Zoom out" onclick={() => zoomBy(1 / 1.2)}><Icon name="minus" size={14} /></button>
      <button type="button" class="pct" title="Back to 100%" onclick={() => (view = { ...view, zoom: 1 })}>{Math.round(view.zoom * 100)}%</button>
      <button type="button" aria-label="Zoom in" onclick={() => zoomBy(1.2)}><Icon name="plus" size={14} /></button>
      <button type="button" class="wide" onclick={fit}>Fit everything</button>
      <span class="sep"></span>
      <button type="button" class="wide" aria-pressed={mode === 'timeline'} onclick={() => void toggleTimeline()}>
        <Icon name="calendar" size={14} />{mode === 'timeline' ? 'Back to my layout' : 'Timeline'}
      </button>
      <button type="button" class="wide" aria-pressed={showLinks} onclick={() => (showLinks = !showLinks)}>
        <Icon name="link" size={14} />Links
      </button>
    </div>
    {#if linkFrom}
      <p class="banner" role="status">Click the card to link with “{titleOf(linkFrom)}” · Esc to cancel</p>
    {:else if mode === 'timeline'}
      <p class="banner" role="status">The timeline: your layout is kept. Back to my layout puts every card where it was.</p>
    {/if}
    <p class="hint">Drag cards · right-click paper for a project area · double-click to jot · Ctrl + scroll to zoom</p>
  </div>

  <aside class="panel" aria-label="Today">
    <button type="button" class="toggle" aria-expanded={panel} onclick={() => (panel = !panel)} title={panel ? 'Hide the panel' : 'Show the panel'}>
      <Icon name={panel ? 'right' : 'left'} size={14} />
    </button>
    {#if panel}
      <div class="stack">
        <div class="paper hours">
          <div><span class="big mono">{fmtHours(today?.minutes ?? 0)}</span><span class="lbl">today</span></div>
          <div><span class="big mono">{fmtHours(week?.minutes ?? 0)}</span><span class="lbl">this week</span></div>
        </div>
        <WhereWasI />
        <LogWork />
        <TrayCard />
        <WrapUp />
      </div>
    {/if}
  </aside>
</div>

<style>
  .desk {
    position: relative;
    height: 100%;
    min-height: 420px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 360px;
    gap: 12px;
  }

  .desk.closed {
    grid-template-columns: minmax(0, 1fr) 28px;
  }

  .board {
    position: relative;
    overflow: hidden;
    border-radius: 10px;
    background-color: var(--desk);
    background-image:
      radial-gradient(circle, color-mix(in srgb, var(--ink) 10%, transparent) 1px, transparent 1.5px),
      var(--grain);
    background-size:
      24px 24px,
      auto;
    box-shadow: inset 0 0 0 1px var(--rule);
    cursor: default;
    touch-action: none;
  }

  .board.panning {
    cursor: grabbing;
  }

  .world {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    transform-origin: 0 0;
  }

  /* While an area is dragged, its cards follow the pointer without easing. */
  .world.moving :global(.card),
  .world.moving :global(.area) {
    transition: none;
  }

  .links {
    position: absolute;
    left: 0;
    top: 0;
    width: 1px;
    height: 1px;
    overflow: visible;
    pointer-events: none;
  }

  .line {
    fill: none;
    stroke: color-mix(in srgb, var(--ink) 30%, transparent);
    stroke-width: 1.5;
    stroke-dasharray: 5 5;
  }

  .line.ticket {
    stroke: color-mix(in srgb, var(--accent) 60%, transparent);
    stroke-dasharray: none;
  }

  .line.drawn {
    stroke: var(--accent);
    stroke-width: 2;
    stroke-dasharray: none;
  }

  .hit {
    fill: none;
    stroke: transparent;
    stroke-width: 14;
    pointer-events: stroke;
    cursor: pointer;
  }

  .day {
    position: absolute;
    top: 40px;
    width: 220px;
    font: 700 11px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
    border-bottom: 2px solid color-mix(in srgb, var(--ink) 15%, transparent);
    padding-bottom: 4px;
  }

  .banner {
    position: absolute;
    left: 50%;
    top: 12px;
    transform: translateX(-50%);
    margin: 0;
    padding: 6px 14px;
    border-radius: 9px;
    background: var(--ink);
    color: var(--paper);
    font-size: 12.5px;
    box-shadow: var(--shadow);
    white-space: nowrap;
    pointer-events: none;
  }

  .tools .sep {
    width: 1px;
    margin: 4px 3px;
    background: var(--rule);
  }

  .tools button[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
  }

  .tools .wide {
    gap: 5px;
    grid-auto-flow: column;
  }

  .jot {
    position: absolute;
    width: 220px;
    font: 13.5px/1.45 var(--ui);
    box-shadow: var(--lift);
    border-top: 3px solid #de9930;
    resize: none;
    z-index: 100000;
  }

  .anchor {
    position: absolute;
    width: 1px;
    height: 1px;
    pointer-events: none;
  }

  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    text-align: center;
    color: var(--mut);
    padding: 24px;
    pointer-events: none;
  }

  .empty p {
    margin: 4px auto;
    max-width: 30rem;
  }

  .empty .big {
    font: 600 22px var(--display);
    color: var(--ink-2);
  }

  .tools {
    position: absolute;
    left: 12px;
    bottom: 12px;
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 9px;
    background: var(--glass);
    box-shadow: var(--shadow);
    backdrop-filter: blur(6px);
  }

  .tools button {
    border: 0;
    background: none;
    color: var(--ink-2);
    min-width: 30px;
    height: 28px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    cursor: pointer;
    font: 600 12px var(--ui);
  }

  .tools button:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink);
  }

  .tools .pct {
    font-family: var(--mono);
    min-width: 46px;
  }

  .tools .wide {
    padding: 0 10px;
  }

  .hint {
    position: absolute;
    right: 12px;
    bottom: 10px;
    margin: 0;
    font-size: 11.5px;
    color: var(--mut);
    pointer-events: none;
  }

  .panel {
    position: relative;
    min-height: 0;
    overflow: auto;
  }

  .toggle {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 2;
    width: 26px;
    height: 26px;
    border: 1px solid var(--rule);
    border-radius: 7px;
    background: var(--paper);
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-left: 34px;
  }

  .hours {
    display: flex;
    gap: 22px;
    padding: 12px 16px;
  }

  .hours > div {
    display: flex;
    flex-direction: column;
  }

  .hours .big {
    font-size: 22px;
    font-weight: 700;
  }

  .hours .lbl {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
  }

  @media (max-width: 900px) {
    .desk {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(420px, 1fr) auto;
    }

    .desk.closed {
      grid-template-columns: 1fr;
    }
  }
</style>
