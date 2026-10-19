<script module lang="ts">
  import { HOME, type CanvasView } from '../lib/canvasView';

  // Where you left the view and the panel, kept while the app is open.
  let lastView: CanvasView = HOME;
  let panelOpen = true;
</script>

<script lang="ts">
  // The Desk: the Canvas where the day happens (round 9). Your entries, Done tickets, notes
  // and Inbox scraps as cards you arrange; only where they sit is new. The panel holds what
  // Day has for logging: today's hours, "Where was I?", Log work, Done tickets and the wrap-up.
  // The full plan is docs/CANVAS.md; this is step 1 (foundation).
  import { onMount, tick, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { CanvasCard as Card, CardKind } from '../lib/api';
  import { fitAll, toCanvas, zoomAt } from '../lib/canvasView';
  import { fmtHours } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { app, attempt, ribbonDay, ribbonWeekOf, selectDay } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import CanvasCard from '../components/CanvasCard.svelte';
  import Icon from '../components/Icon.svelte';
  import LogWork from '../components/LogWork.svelte';
  import TrayCard from '../components/TrayCard.svelte';
  import WhereWasI from '../components/WhereWasI.svelte';
  import WrapUp from '../components/WrapUp.svelte';

  let cards = $state<Card[]>([]);
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

  // N (or anything that asks for Log work) opens the panel if it was folded away.
  $effect(() => {
    if (app.logFocus.serial > 0) untrack(() => (panel = true));
  });

  const today = $derived(ribbonDay(app.today));
  const week = $derived(ribbonWeekOf(app.today));

  async function load() {
    const list = await attempt(() => api.canvas(app.today));
    if (list) cards = list;
    loaded = true;
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
    cards = cards.map((c) => (c === card ? { ...c, x, y, z: top, placed: true } : c));
    await attempt(() => api.canvasMove(card.kind, card.id, x, y));
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
    view = fitAll(cards, board.clientWidth, board.clientHeight);
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
    openMenu(menuAnchor, {
      sections: [
        {
          head: 'The Desk',
          items: [
            { label: 'New jot here', icon: 'note', side: true, act: () => startJot(sx, sy) },
            { label: 'Fit everything', icon: 'open', side: true, act: fit },
            { label: 'Back to 100%', icon: 'undo', side: true, act: () => (view = { ...view, zoom: 1 }) },
          ],
        },
      ],
    });
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
  >
    <div class="world" style:transform="translate({view.x}px, {view.y}px) scale({view.zoom})">
      {#each cards as card (card.kind + card.id)}
        <CanvasCard {card} zoom={view.zoom} onmove={(x, y) => void move(card, x, y)} onchanged={() => void load()} />
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
    </div>
    <p class="hint">Drag cards · double-click paper to jot · Ctrl + scroll to zoom</p>
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
