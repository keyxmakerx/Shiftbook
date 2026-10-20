<script module lang="ts">
  import { HOME, type CanvasView } from '../lib/canvasView';

  // Where you left the view, the panel, the links switch and the toolbox, kept while the app
  // is open.
  let lastView: CanvasView = HOME;
  let panelOpen = true;
  let linksShown = true;
  let lastTool: 'hand' | 'select' | 'box' = 'hand';
  let toolboxOpen = true;
  let cabinetShown = false;
</script>

<script lang="ts">
  // The Desk: the Canvas where the day happens (round 9). Your entries, Done tickets, notes
  // and Inbox scraps as cards you arrange; only where they sit is new. The panel holds what
  // Day has for logging: today's hours, "Where was I?", Log work, Done tickets and the wrap-up.
  // Step 2 adds project areas (drop a card in to give it the project), links between cards
  // (automatic and drawn) and the timeline. Step 3 (round 12) adds the toolbox: the hand
  // pans, Select rubber-bands cards that then move together, Box (or Shift + drag anywhere)
  // draws a box that becomes a project's area or a plain labelled box; boxes fold up to a
  // coloured count. Step 4 adds piles (drop a card on a card; click to fan out) and the
  // cabinet of drawers (drag a card or pile onto one to file it off the Desk), plus Tidy into
  // stacks. The full plan is docs/CANVAS.md.
  import { onMount, tick, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type {
    CanvasArea,
    CanvasCard as Card,
    CanvasDrawer,
    CanvasLabel,
    CanvasLink,
    CanvasStack,
    CardKind,
    CardRef,
  } from '../lib/api';
  import {
    areaAt,
    boxAround,
    cardIn,
    cardUnder,
    CARD_H,
    CARD_W,
    deskBoxes,
    drawerAt,
    fanSpots,
    fitAll,
    MIN_AREA,
    NEW_AREA,
    rectFrom,
    toCanvas,
    zoomAt,
    type DeskBox,
    type Rect,
  } from '../lib/canvasView';
  import { input as entryInput, moveEntryTo } from '../lib/entryActions';
  import { dayMonth, fmtHours, weekdayShort } from '../lib/format';
  import { openMenu, type MenuSpec } from '../lib/menu.svelte';
  import { openBoard } from '../lib/boards.svelte';
  import { closePanel } from '../lib/panel.svelte';
  import { app, attempt, projectById, refresh, refreshProjects, ribbonDay, ribbonWeekOf, savePref, selectDay } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import BoxPrompt from '../components/BoxPrompt.svelte';
  import Cabinet from '../components/Cabinet.svelte';
  import CanvasPile from '../components/CanvasPile.svelte';
  import DrawerPanel from '../components/DrawerPanel.svelte';
  import CanvasAreaBox from '../components/CanvasArea.svelte';
  import CanvasCard from '../components/CanvasCard.svelte';
  import Icon from '../components/Icon.svelte';
  import LogWork from '../components/LogWork.svelte';
  import TrayCard from '../components/TrayCard.svelte';
  import WhereWasI from '../components/WhereWasI.svelte';
  import WrapUp from '../components/WrapUp.svelte';

  let cards = $state<Card[]>([]);
  let areas = $state<CanvasArea[]>([]);
  let labels = $state<CanvasLabel[]>([]);
  let stacks = $state<CanvasStack[]>([]);
  let drawers = $state<CanvasDrawer[]>([]);
  /** The pile spread out beside itself, if any. */
  let fanned = $state<string | null>(null);
  /** The drawer or the card something is being dragged over. */
  let hoverDrawer = $state<string | null>(null);
  let hoverCard = $state<string | null>(null);
  let openDrawer = $state<string | null>(null);
  let renamingPile = $state<string | null>(null);
  let cabinet = $state(cabinetShown);
  /** A card or pile is being dragged: the cabinet slides open to take it. */
  let dragging = $state(false);
  let links = $state<CanvasLink[]>([]);
  let axis = $state<{ date: string; x: number }[]>([]);
  let mode = $state<'free' | 'timeline'>('free');
  let viewBeforeTimeline: CanvasView | null = null;
  let showLinks = $state(linksShown);
  let linkFrom = $state<Card | null>(null);
  let areaDrag = $state<{ index: number; dx: number; dy: number; members: Set<string>; piles: Set<string> } | null>(null);
  let tool = $state(lastTool);
  let toolbox = $state(toolboxOpen);
  /** Cards picked with Select; dragging one moves them all. */
  let selected = $state(new Set<string>());
  /** The selected card being dragged and how far, so the others follow. */
  let groupDrag = $state<{ key: string; dx: number; dy: number } | null>(null);
  /** A rubber band (Select) or a box being drawn (Box / Shift), in board pixels. */
  let band = $state<{ x0: number; y0: number; x1: number; y1: number; drawing: boolean } | null>(null);
  /** A box just drawn, waiting for "What is this box?". */
  let drawn = $state<{ rect: Rect; sx: number; sy: number } | null>(null);
  let renaming = $state<string | null>(null);
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
  $effect(() => {
    lastTool = tool;
    toolboxOpen = toolbox;
    cabinetShown = cabinet;
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
      labels = desk.labels;
      stacks = desk.stacks;
      drawers = desk.drawers;
      if (fanned && !stacks.some((st) => st.id === fanned)) fanned = null;
      if (openDrawer && !drawers.some((d) => d.key === openDrawer)) openDrawer = null;
      links = desk.links;
      axis = desk.axis;
    }
    loaded = true;
  }

  const keyOf = (c: { kind: CardKind; id: string }) => `${c.kind}:${c.id}`;
  const refOf = (c: Card): CardRef => ({ kind: c.kind, id: c.id });

  /** Where a card shows: its own spot, its timeline spot, and moved along with an area being
   *  dragged or the selected card being dragged. */
  function pos(c: Card): { x: number; y: number } {
    if (mode === 'timeline') return { x: c.timelineX, y: c.timelineY };
    if (c.stack && c.stack === fanned) {
      const pile = stacks.find((st) => st.id === c.stack)!;
      const members = pileMembers(c.stack);
      return fanSpots(pile.x, pile.y, members.length)[members.indexOf(c)] ?? { x: c.x, y: c.y };
    }
    if (areaDrag?.members.has(keyOf(c))) return { x: c.x + areaDrag.dx, y: c.y + areaDrag.dy };
    const k = keyOf(c);
    if (groupDrag && groupDrag.key !== k && selected.has(k)) return { x: c.x + groupDrag.dx, y: c.y + groupDrag.dy };
    return { x: c.x, y: c.y };
  }

  // ----- boxes -----
  const boxes = $derived(deskBoxes(areas, labels));
  /** The box a card sits in now (as it shows, so moves count straight away). */
  const boxOf = (c: Card): DeskBox | undefined => boxes[areaAt(boxes, c.x, c.y)];
  /** Cards in a folded box don't show, nor cards in a pile unless it's fanned out (the timeline
   *  shows everything). */
  const shown = $derived(
    mode === 'timeline' ? cards : cards.filter((c) => !boxOf(c)?.collapsed && (!c.stack || c.stack === fanned)),
  );

  // ----- piles -----
  /** A pile's cards, bottom first (the last is on top). */
  const pileMembers = (id: string) => cards.filter((c) => c.stack === id).sort((a, b) => a.z - b.z);
  const piles = $derived(mode === 'timeline' ? [] : stacks.filter((st) => !boxes[areaAt(boxes, st.x, st.y)]?.collapsed));
  const pileAt = (st: CanvasStack) =>
    areaDrag && areaDrag.piles.has(st.id) ? { ...st, x: st.x + areaDrag.dx, y: st.y + areaDrag.dy } : st;

  /** Drop targets for a card: other cards showing, and piles (by their top card). */
  function dropTarget(card: Card, x: number, y: number): { ref: CardRef; x: number; y: number; key: string } | null {
    // The Done today pile only takes tickets as they're ticked done, not dropped cards.
    const targets = [
      ...piles.filter((st) => !st.done).map((st) => {
        const top = pileMembers(st.id).at(-1)!;
        return { ref: refOf(top), x: st.x, y: st.y, key: `pile:${st.id}` };
      }),
      ...shown.filter((c) => keyOf(c) !== keyOf(card) && !(card.stack && c.stack === card.stack) && c.stack !== 'done').map((c) => {
        const p = pos(c);
        return { ref: refOf(c), x: p.x, y: p.y, key: keyOf(c) };
      }),
    ];
    const i = cardUnder(targets, x, y);
    return i >= 0 ? targets[i]! : null;
  }

  function cardDragging(card: Card, dx: number, dy: number, at: { cx: number; cy: number }) {
    dragging = true;
    if (selected.has(keyOf(card)) && selected.size > 1) groupDrag = { key: keyOf(card), dx, dy };
    hoverDrawer = drawerAt(at.cx, at.cy);
    const p = pos(card);
    hoverCard = hoverDrawer || groupDrag ? null : (dropTarget(card, p.x + dx, p.y + dy)?.key ?? null);
  }

  async function pileMove(st: CanvasStack, x: number, y: number, at: { cx: number; cy: number }) {
    const drawer = drawerAt(at.cx, at.cy);
    hoverDrawer = null;
    dragging = false;
    if (drawer) return fileThese(pileMembers(st.id), drawer, st.done ? undefined : st);
    stacks = stacks.map((p) => (p.id === st.id ? { ...p, x, y } : p));
    if (await movePile(st, x, y)) await load();
  }

  /** Save where a pile sits: the Done today pile's spot is a preference. */
  async function movePile(st: CanvasStack, x: number, y: number): Promise<boolean> {
    if (st.done) return !!(await savePref('desk.done_pile', `${Math.round(x)} ${Math.round(y)}`));
    return !!(await attempt(() => api.canvasStackMove(st.id, x, y).then(() => true)));
  }

  async function renamePile(st: CanvasStack, name: string) {
    if (await attempt(() => api.canvasStackRename(st.id, name).then(() => true))) await load();
  }

  function pileMenu(st: CanvasStack): MenuSpec {
    const fan = { label: fanned === st.id ? 'Put them back in the pile' : 'Fan them out', icon: 'open' as const, side: true, act: () => (fanned = fanned === st.id ? null : st.id) };
    if (st.done) {
      return {
        sections: [
          {
            head: `Done today · ${st.count}`,
            items: [
              fan,
              { label: 'See all done tickets', icon: 'tray', side: true, act: () => openBoard('done', 'list') },
            ],
          },
          ...fileSection((key) => void fileThese(pileMembers(st.id), key)),
        ],
      };
    }
    return {
      sections: [
        {
          head: st.name ?? `A pile of ${st.count}`,
          items: [
            { label: fanned === st.id ? 'Put them back in the pile' : 'Fan them out', icon: 'open', side: true, act: () => (fanned = fanned === st.id ? null : st.id) },
            { label: 'Rename', icon: 'edit', side: true, act: () => (renamingPile = st.id) },
            {
              label: 'Take the pile apart',
              icon: 'x',
              top: true,
              act: () => void attempt(() => api.canvasStackDissolve(st.id).then(() => true)).then((ok) => {
                if (ok) void load();
              }),
            },
          ],
        },
        ...fileSection((key) => void fileThese(pileMembers(st.id), key, st)),
      ],
    };
  }

  // ----- drawers -----
  /** The "File in" choices for a card's or a pile's menu. */
  function fileSection(file: (key: string) => void): MenuSpec['sections'] {
    if (!drawers.length) return [];
    return [{ head: 'File in a drawer', choices: drawers.map((d) => ({ label: d.name, color: d.color, checked: false, act: () => file(d.key) })) }];
  }

  /** File cards (one, a selection or a pile) in a drawer. A project's drawer also gives
   *  them the project. One Undo puts everything back. */
  async function fileThese(list: Card[], key: string, pile?: CanvasStack) {
    if (!list.length) return;
    const was = list.map((c) => ({ ref: refOf(c), x: c.x, y: c.y }));
    const ok = pile
      ? await attempt(() => api.canvasFileStack(pile.id, key).then(() => true))
      : await attempt(async () => {
          for (const c of list) await api.canvasFile(refOf(c), key);
          return true;
        });
    if (!ok) return;
    const projectId = key.startsWith('project:') ? key.slice('project:'.length) : null;
    const undoProject = projectId ? await giveProject(list, projectId) : [];
    selected = new Set();
    if (fanned === pile?.id) fanned = null;
    if (projectId) await refresh();
    await load();
    const name = drawers.find((d) => d.key === key)?.name ?? 'the drawer';
    toast(`Filed ${list.length === 1 ? 'it' : `${list.length} cards`} in ${name}`, 'Undo', () =>
      void (async () => {
        for (const w of was) await attempt(() => api.canvasUnfile(w.ref, w.x, w.y));
        for (const u of undoProject) await attempt(u);
        await refresh();
        await load();
      })(),
    );
  }

  async function addFolder(name: string) {
    if (await attempt(() => api.canvasFolderSave(null, name))) await load();
  }

  async function renameFolder(key: string, name: string) {
    if (await attempt(() => api.canvasFolderSave(key.slice('folder:'.length), name))) await load();
  }

  async function removeFolder(key: string) {
    const id = key.slice('folder:'.length);
    const name = drawers.find((d) => d.key === key)?.name ?? 'the folder';
    if (!(await attempt(() => api.canvasFolderRemove(id, true).then(() => true)))) return;
    openDrawer = null;
    await load();
    toast(`Removed ${name}. Its cards are back on the Desk.`, 'Undo', () => {
      void attempt(() => api.canvasFolderRemove(id, false)).then(load);
    });
  }

  /** Where a card put back from a drawer lands: the middle of what you're looking at. */
  function middle() {
    const at = toCanvas(view, (board?.clientWidth ?? 800) / 2, (board?.clientHeight ?? 600) / 2);
    return { x: Math.round(at.x - CARD_W / 2), y: Math.round(at.y - CARD_H / 2) };
  }

  /** The Desk's own menu sections for a card: file it. */
  const cardExtra = (card: Card) => () => fileSection((key) => void fileThese(selected.has(keyOf(card)) ? cards.filter((c) => selected.has(keyOf(c))) : [card], key));

  async function tidy() {
    const done = await attempt(() => api.canvasTidy(app.today));
    if (!done) return;
    await load();
    if (!done.stacks.length) {
      toast('Nothing to tidy: no project has two loose cards.');
      return;
    }
    toast(`Made ${done.stacks.length} pile${done.stacks.length === 1 ? '' : 's'}, one per project`, 'Undo', () =>
      void (async () => {
        for (const id of done.stacks) await attempt(() => api.canvasStackDissolve(id));
        for (const b of done.before) await attempt(() => api.canvasMove(b.kind, b.id, b.x, b.y));
        await load();
      })(),
    );
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
  async function move(card: Card, x: number, y: number, at?: { cx: number; cy: number }) {
    const from = pos(card);
    const dx = x - from.x;
    const dy = y - from.y;
    const grouped = !!groupDrag;
    groupDrag = null;
    hoverCard = null;
    // A selected card brings the rest of the selection along.
    const group = selected.has(keyOf(card)) ? cards.filter((c) => c !== card && selected.has(keyOf(c))) : [];
    // Dropped on a drawer: filed (with the rest of the selection).
    const drawer = at ? drawerAt(at.cx, at.cy) : null;
    hoverDrawer = null;
    dragging = false;
    if (drawer) return fileThese([card, ...group], drawer);
    // Dropped on another card or a pile: they pile up.
    const target = grouped ? null : dropTarget(card, x, y);
    if (target) {
      if (!(await attempt(() => api.canvasStack(refOf(card), target.ref, target.x, target.y).then(() => true)))) return;
      await load();
      toast('Piled up. Click the pile to fan it out; drag a card away to take it off.');
      return;
    }
    await place(card, x, y);
    for (const c of group) await place(c, c.x + dx, c.y + dy);
    // A card dragged out of a fanned pile is off it now.
    if (card.stack) await load();
  }

  async function place(card: Card, x: number, y: number) {
    const top = Math.max(0, ...cards.map((c) => c.z)) + 1;
    const before = { x: card.x, y: card.y };
    cards = cards.map((c) => (keyOf(c) === keyOf(card) ? { ...c, x, y, z: top, placed: true } : c));
    await attempt(() => api.canvasMove(card.kind, card.id, x, y));
    // Into a project's area gives the card that project; out of its own area takes it away.
    // Labelled boxes only group cards.
    const toProject = boxes[areaAt(boxes, x, y)]?.projectId ?? null;
    const fromProject = boxes[areaAt(boxes, before.x, before.y)]?.projectId ?? null;
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

  // ----- boxes: project areas and labelled boxes -----
  /** Save a box where it is now (a project's area or a labelled box). */
  function saveBox(b: DeskBox) {
    return b.projectId
      ? api.canvasArea(b.projectId, b.x, b.y, b.w, b.h)
      : api.canvasLabelSave(b.labelId, b.label, b.x, b.y, b.w, b.h, b.collapsed).then(() => undefined);
  }

  /** Show a box's change at once, before the Desk reloads. */
  function putBox(b: DeskBox) {
    if (b.projectId) areas = areas.map((a) => (a.projectId === b.projectId ? { ...a, x: b.x, y: b.y, w: b.w, h: b.h, collapsed: b.collapsed } : a));
    else labels = labels.map((l) => (l.id === b.labelId ? { ...l, label: b.label, x: b.x, y: b.y, w: b.w, h: b.h, collapsed: b.collapsed } : l));
  }

  function areaMove(index: number, dx: number, dy: number, done: boolean) {
    if (!areaDrag) {
      const members = new Set(cards.filter((c) => !c.stack && areaAt(boxes, c.x, c.y) === index).map(keyOf));
      const inside = new Set(stacks.filter((st) => areaAt(boxes, st.x, st.y) === index).map((st) => st.id));
      areaDrag = { index, dx, dy, members, piles: inside };
    } else areaDrag = { ...areaDrag, dx, dy };
    if (!done) return;
    const drag = areaDrag;
    areaDrag = null;
    const b = boxes[index]!;
    if (drag.dx === 0 && drag.dy === 0) return;
    const moved = { ...b, x: b.x + drag.dx, y: b.y + drag.dy };
    putBox(moved);
    cards = cards.map((c) => (drag.members.has(keyOf(c)) ? { ...c, x: c.x + drag.dx, y: c.y + drag.dy, placed: true } : c));
    stacks = stacks.map((st) => (drag.piles.has(st.id) ? { ...st, x: st.x + drag.dx, y: st.y + drag.dy } : st));
    void (async () => {
      await attempt(() => saveBox(moved));
      for (const c of cards.filter((c) => drag.members.has(keyOf(c)))) {
        await attempt(() => api.canvasMove(c.kind, c.id, c.x, c.y));
      }
      for (const st of stacks.filter((st) => drag.piles.has(st.id))) await movePile(st, st.x, st.y);
      await load();
    })();
  }

  async function areaResize(index: number, w: number, h: number) {
    const b = { ...boxes[index]!, w, h };
    putBox(b);
    await attempt(() => saveBox(b));
  }

  async function toggleBox(b: DeskBox) {
    const next = { ...b, collapsed: !b.collapsed };
    putBox(next);
    if (b.projectId) await attempt(() => api.canvasAreaCollapse(b.projectId!, next.collapsed));
    else await attempt(() => saveBox(next));
    await load();
  }

  async function renameBox(b: DeskBox, label: string) {
    const next = { ...b, label };
    putBox(next);
    if (await attempt(() => saveBox(next).then(() => true))) await load();
  }

  async function removeLabel(b: DeskBox) {
    const id = b.labelId!;
    if (!(await attempt(() => api.canvasLabelRemove(id, true).then(() => true)))) return;
    await load();
    toast(`Took “${b.label}” off the Desk. Its cards stay.`, 'Undo', () => {
      void attempt(() => api.canvasLabelRemove(id, false)).then(load);
    });
  }

  async function addArea(projectId: string, x: number, y: number) {
    if (!(await attempt(() => api.canvasArea(projectId, x, y, NEW_AREA.w, NEW_AREA.h).then(() => true)))) return;
    await load();
    toast(`Drop cards into ${projectById(projectId)?.name ?? 'the area'} to give them that project.`);
  }

  function boxMenu(b: DeskBox): MenuSpec {
    const fold = { label: b.collapsed ? 'Open it out' : 'Fold it up', icon: b.collapsed ? ('down' as const) : ('up' as const), side: true, act: () => void toggleBox(b) };
    if (!b.projectId) {
      return {
        sections: [
          {
            head: b.label,
            items: [
              fold,
              { label: 'Rename', icon: 'edit', side: true, act: () => (renaming = b.key) },
              { label: 'Take the box off the Desk', icon: 'trash', danger: true, top: true, act: () => void removeLabel(b) },
            ],
          },
        ],
      };
    }
    const a = areas.find((x) => x.projectId === b.projectId)!;
    const p = projectById(a.projectId);
    return {
      sections: [
        {
          head: `${p?.name ?? 'Project'} area`,
          items: [
            { label: 'Open full screen', icon: 'expand', side: true, act: () => openBoard(`project:${a.projectId}`) },
            fold,
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
  // Only cards that show have lines (a folded box hides its cards' lines too).
  const centres = $derived(
    new Map(shown.map((c) => {
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
    if ((e.target as HTMLElement).closest('.tools, .jot, .rail, .selbar, .drawer-at')) return;
    fanned = null;
    closePanel();
    if (mode === 'free' && (tool === 'box' || tool === 'select' || e.shiftKey)) {
      bandStart(e, tool === 'box' || e.shiftKey);
      return;
    }
    if (selected.size) selected = new Set();
    // Dragging paper mustn't select text: a selection left behind gets dragged by the browser
    // itself the next time a card is grabbed on it, and the card sticks.
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    // That also stops the press moving focus, so a rename box being typed in is let go here.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
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

  // ----- Select and Box: a rubber band on empty paper -----
  function bandStart(e: PointerEvent, drawing: boolean) {
    // Shift + drag would otherwise select text on the page.
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    const { sx, sy } = boardPoint(e);
    band = { x0: sx, y0: sy, x1: sx, y1: sy, drawing };
    board!.setPointerCapture(e.pointerId);
    const moveFn = (m: PointerEvent) => {
      const p = boardPoint(m);
      if (band) band = { ...band, x1: p.sx, y1: p.sy };
    };
    const up = () => {
      board?.removeEventListener('pointermove', moveFn);
      board?.removeEventListener('pointerup', up);
      board?.removeEventListener('pointercancel', up);
      const b = band;
      band = null;
      if (b) bandDone(b);
    };
    board!.addEventListener('pointermove', moveFn);
    board!.addEventListener('pointerup', up);
    board!.addEventListener('pointercancel', up);
  }

  function bandDone(b: { x0: number; y0: number; x1: number; y1: number; drawing: boolean }) {
    const rect = rectFrom(toCanvas(view, b.x0, b.y0), toCanvas(view, b.x1, b.y1));
    if (!b.drawing) {
      selected = new Set(shown.filter((c) => cardIn(rect, c.x, c.y)).map(keyOf));
      return;
    }
    // Too small to be meant as a box: a click, not a drag.
    if (rect.w * view.zoom < 40 || rect.h * view.zoom < 40) return;
    const sized = { ...rect, w: Math.max(MIN_AREA.w, Math.round(rect.w)), h: Math.max(MIN_AREA.h, Math.round(rect.h)), x: Math.round(rect.x), y: Math.round(rect.y) };
    askBox(sized, Math.max(b.x0, b.x1) + 8, Math.min(b.y0, b.y1));
  }

  function askBox(rect: Rect, sx: number, sy: number) {
    const w = board?.clientWidth ?? 800;
    const h = board?.clientHeight ?? 600;
    drawn = { rect, sx: Math.min(Math.max(8, sx), w - 310), sy: Math.min(Math.max(8, sy), h - 260) };
  }

  /** "Box these": a box just around the selected cards. */
  function boxSelection() {
    const picked = cards.filter((c) => selected.has(keyOf(c)));
    if (!picked.length) return;
    const rect = boxAround(picked);
    const corner = { x: (rect.x + rect.w) * view.zoom + view.x, y: rect.y * view.zoom + view.y };
    askBox(rect, corner.x + 8, corner.y);
  }

  const insideDrawn = $derived(drawn ? cards.filter((c) => cardIn(drawn!.rect, c.x, c.y)) : []);

  /** Give cards a project together, with one Undo for all of them. */
  async function giveProject(list: Card[], projectId: string) {
    const undo: (() => Promise<unknown>)[] = [];
    for (const c of list) {
      if (c.entry && c.entry.projectId !== projectId && !c.entry.isTray) {
        const e = c.entry;
        if (await attempt(() => api.updateEntry(e.id, entryInput(e, { projectId })))) {
          undo.push(() => api.updateEntry(e.id, entryInput(e, {})));
        }
      } else if (c.note && c.note.projectId !== projectId) {
        const n = c.note;
        const write = (pid: string | null) =>
          api.updateNote(n.id, { kind: n.kind, title: n.title, body: n.body, projectId: pid, tags: n.tags.join(', ') });
        if (await attempt(() => write(projectId))) undo.push(() => write(n.projectId));
      } else if (c.tray && c.tray.item.projectId !== projectId) {
        const t = c.tray.item;
        if (await attempt(() => api.traySetProject(t.id, projectId))) undo.push(() => api.traySetProject(t.id, t.projectId));
      }
    }
    return undo;
  }

  async function boxForProject(projectId: string) {
    const d = drawn;
    if (!d) return;
    const inside = cards.filter((c) => cardIn(d.rect, c.x, c.y));
    drawn = null;
    if (!(await attempt(() => api.canvasArea(projectId, d.rect.x, d.rect.y, d.rect.w, d.rect.h).then(() => true)))) return;
    const undo = await giveProject(inside, projectId);
    selected = new Set();
    await refresh();
    await load();
    const name = projectById(projectId)?.name ?? 'the project';
    toast(undo.length ? `${undo.length} card${undo.length === 1 ? '' : 's'} now in ${name}` : `Box for ${name}`, undo.length ? 'Undo' : undefined, undo.length
      ? () => void (async () => {
          for (const u of undo) await attempt(u);
          await refresh();
          await load();
        })()
      : undefined);
  }

  async function boxForNewProject(name: string) {
    const made = await attempt(() => api.createProject(name, null));
    if (!made) return;
    await refreshProjects();
    await boxForProject(made.id);
  }

  async function boxWithLabel(label: string) {
    const d = drawn;
    drawn = null;
    if (!d) return;
    if (!(await attempt(() => api.canvasLabelSave(null, label, d.rect.x, d.rect.y, d.rect.w, d.rect.h, false)))) return;
    selected = new Set();
    await load();
    toast(`Box “${label}”. Drag its name to move it with its cards; ▴ folds it up.`);
  }

  $effect(() => {
    if (!selected.size) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !drawn) {
        e.stopPropagation();
        selected = new Set();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });

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
    const spots = [...shown.map(pos), ...(mode === 'free' ? boxes.flatMap((a) => [{ x: a.x, y: a.y }, { x: a.x + a.w - CARD_W, y: a.y + a.h - CARD_H }]) : [])];
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
          ...(mode === 'free'
            ? [{ label: 'Draw a box (or Shift + drag)', icon: 'folder' as const, top: true, act: () => (tool = 'box') }]
            : []),
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
    class:drawing={mode === 'free' && tool !== 'hand'}
    bind:this={board}
    onpointerdown={panStart}
    ondragstart={(e) => e.preventDefault()}
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
        {#each boxes as b, i (b.key)}
          <CanvasAreaBox
            box={areaDrag?.index === i ? { ...b, x: b.x + areaDrag.dx, y: b.y + areaDrag.dy } : b}
            zoom={view.zoom}
            onmove={(dx, dy, done) => areaMove(i, dx, dy, done)}
            onresize={(w, h) => void areaResize(i, w, h)}
            ontoggle={() => void toggleBox(b)}
            onrename={(label) => void renameBox(b, label)}
            bind:renaming={() => renaming === b.key, (v) => (renaming = v ? b.key : null)}
            menu={() => boxMenu(b)}
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
      {#each piles as st (st.id)}
        {@const top = pileMembers(st.id).at(-1)}
        {#if top}
          <CanvasPile
            pile={pileAt(st)}
            {top}
            zoom={view.zoom}
            fanned={fanned === st.id}
            dropping={hoverCard === `pile:${st.id}`}
            onmove={(x, y, at) => void pileMove(st, x, y, at)}
            ondrag={(at) => {
              dragging = true;
              hoverDrawer = drawerAt(at.cx, at.cy);
            }}
            ontoggle={() => (fanned = fanned === st.id ? null : st.id)}
            onrename={(name) => void renamePile(st, name)}
            bind:renaming={() => renamingPile === st.id, (v) => (renamingPile = v && !st.done ? st.id : null)}
            menu={() => pileMenu(st)}
          />
        {/if}
      {/each}
      {#each shown as card (card.kind + card.id)}
        {@const p = pos(card)}
        <CanvasCard
          {card}
          x={p.x}
          y={p.y}
          zoom={view.zoom}
          locked={mode === 'timeline'}
          linking={!!linkFrom}
          selected={selected.has(keyOf(card))}
          dropping={hoverCard === keyOf(card)}
          extra={cardExtra(card)}
          ondrag={(dx, dy, at) => cardDragging(card, dx, dy, at)}
          onmove={(x, y, at) => void move(card, x, y, at)}
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
    {#if band}
      {@const r = rectFrom({ x: band.x0, y: band.y0 }, { x: band.x1, y: band.y1 })}
      <span class="band" class:drawing={band.drawing} style:left="{r.x}px" style:top="{r.y}px" style:width="{r.w}px" style:height="{r.h}px"></span>
    {/if}
    {#if drawn}
      {@const r = drawn.rect}
      <span
        class="band drawing held"
        style:left="{r.x * view.zoom + view.x}px"
        style:top="{r.y * view.zoom + view.y}px"
        style:width="{r.w * view.zoom}px"
        style:height="{r.h * view.zoom}px"
      ></span>
      <BoxPrompt
        x={drawn.sx}
        y={drawn.sy}
        inside={insideDrawn.length}
        boxed={new Set(areas.map((a) => a.projectId))}
        onproject={(id) => void boxForProject(id)}
        onnew={(name) => void boxForNewProject(name)}
        onlabel={(label) => void boxWithLabel(label)}
        oncancel={() => (drawn = null)}
      />
    {/if}

    {#if mode === 'free'}
      <div class="rail">
      <div class="toolbox" class:shut={!toolbox} role="toolbar" aria-label="Desk tools" aria-orientation="vertical">
        <button type="button" class="tb fold" title={toolbox ? 'Fold the toolbox away' : 'Open the toolbox'} aria-expanded={toolbox} onclick={() => (toolbox = !toolbox)}>
          <Icon name={toolbox ? 'left' : 'right'} size={13} />
        </button>
        {#if toolbox}
          <button type="button" class="tb" aria-pressed={tool === 'hand'} title="Move around: drag empty paper to pan" onclick={() => (tool = 'hand')}>
            <Icon name="hand" size={16} /><span>Move</span>
          </button>
          <button type="button" class="tb" aria-pressed={tool === 'select'} title="Select: drag around cards, then move them together" onclick={() => (tool = 'select')}>
            <Icon name="select" size={16} /><span>Select</span>
          </button>
          <button type="button" class="tb" aria-pressed={tool === 'box'} title="Box: drag to draw a box for a project or a label (Shift + drag works with any tool)" onclick={() => (tool = 'box')}>
            <Icon name="box" size={16} /><span>Box</span>
          </button>
        {/if}
      </div>
      <Cabinet {drawers} hover={hoverDrawer} bind:open={cabinet} peek={dragging} opened={openDrawer} onopen={(k) => (openDrawer = openDrawer === k ? null : k)} onaddfolder={(n) => void addFolder(n)} />
      </div>
      {#if openDrawer}
        {@const d = drawers.find((x) => x.key === openDrawer)}
        {#if d}
          <div class="drawer-at">
            <DrawerPanel
              drawer={d}
              putBack={middle}
              onclose={() => (openDrawer = null)}
              onchanged={() => void load()}
              onrename={(name) => void renameFolder(d.key, name)}
              onremove={() => void removeFolder(d.key)}
            />
          </div>
        {/if}
      {/if}
    {/if}
    {#if selected.size}
      <div class="selbar" role="toolbar" aria-label="Selection">
        <span>{selected.size} selected · drag one to move them all</span>
        <button type="button" class="btn" onclick={boxSelection}><Icon name="box" size={13} /> Box these</button>
        <button type="button" class="btn" onclick={() => (selected = new Set())}>Clear</button>
      </div>
    {/if}

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
      <button type="button" class="wide" title="Step into a project, a meeting, the Daily notes, a folder or the Archive" onclick={() => openBoard('desk')}>
        <Icon name="expand" size={14} />Boards
      </button>
      {#if mode === 'free'}
        <button type="button" class="wide" title="Pile loose cards by project" onclick={() => void tidy()}>
          <Icon name="box" size={14} />Tidy into stacks
        </button>
      {/if}
    </div>
    {#if linkFrom}
      <p class="banner" role="status">Click the card to link with “{titleOf(linkFrom)}” · Esc to cancel</p>
    {:else if mode === 'timeline'}
      <p class="banner" role="status">The timeline: your layout is kept. Back to my layout puts every card where it was.</p>
    {/if}
    <p class="hint">Drag cards · Shift + drag to draw a box · double-click to jot · Ctrl + scroll to zoom</p>
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

  .board.drawing {
    cursor: crosshair;
    user-select: none;
  }

  .band {
    position: absolute;
    border: 1.5px dashed var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border-radius: 4px;
    pointer-events: none;
    z-index: 3;
  }

  .band.drawing {
    border-radius: 14px;
    border-width: 2px;
  }

  .rail {
    position: absolute;
    left: 12px;
    top: 12px;
    bottom: 56px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    z-index: 4;
    pointer-events: none;
  }

  .rail > :global(*) {
    pointer-events: auto;
  }

  .drawer-at {
    position: absolute;
    left: 194px;
    top: 12px;
    bottom: 56px;
    z-index: 6;
    display: flex;
    align-items: flex-start;
  }

  .toolbox {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    border-radius: 11px;
    background: var(--glass);
    box-shadow: var(--shadow);
    backdrop-filter: blur(6px);
    z-index: 4;
  }

  .toolbox .tb {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 52px;
    padding: 6px 0 4px;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    font: 600 10.5px var(--ui);
    cursor: pointer;
  }

  .toolbox .tb.fold {
    padding: 3px 0;
  }

  .toolbox.shut .tb.fold {
    width: 26px;
  }

  .toolbox .tb:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink);
  }

  .toolbox .tb[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }

  .selbar {
    position: absolute;
    left: 50%;
    bottom: 56px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 14px;
    border-radius: 11px;
    background: var(--paper);
    box-shadow: var(--lift);
    font-size: 13px;
    z-index: 5;
    white-space: nowrap;
  }

  .selbar .btn {
    padding: 4px 10px;
    font-size: 12.5px;
    display: inline-flex;
    gap: 5px;
    align-items: center;
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
