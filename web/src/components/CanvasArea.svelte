<script lang="ts">
  // A project's area on the Desk (Canvas step 2): a tinted region. Cards dropped inside get the
  // project; dragging the name moves the area and the cards in it; the corner resizes it.
  // Right-click for its paper menu.
  import type { CanvasArea } from '../lib/api';
  import { MIN_AREA } from '../lib/canvasView';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { projectById, projectColor } from '../lib/store.svelte';

  let {
    area,
    zoom,
    onmove,
    onresize,
    menu,
  }: {
    area: CanvasArea;
    zoom: number;
    /** Moving (live, `done` false) or dropped (`done` true), by this much in canvas units. */
    onmove: (dx: number, dy: number, done: boolean) => void;
    /** Resized to this size (on letting go). */
    onresize: (w: number, h: number) => void;
    menu: () => MenuSpec;
  } = $props();

  let size = $state<{ w: number; h: number } | null>(null);

  const name = $derived(projectById(area.projectId)?.name ?? 'Project');
  const color = $derived(projectColor(area.projectId));

  function track(e: PointerEvent, step: (dx: number, dy: number) => void, end: (dx: number, dy: number) => void) {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    const start = { x: e.clientX, y: e.clientY };
    let last = { dx: 0, dy: 0 };
    target.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      last = { dx: (m.clientX - start.x) / zoom, dy: (m.clientY - start.y) / zoom };
      step(last.dx, last.dy);
    };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
      end(last.dx, last.dy);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }

  const dragName = (e: PointerEvent) =>
    track(
      e,
      (dx, dy) => onmove(dx, dy, false),
      (dx, dy) => onmove(Math.round(dx), Math.round(dy), true),
    );

  const dragCorner = (e: PointerEvent) =>
    track(
      e,
      (dx, dy) => (size = { w: Math.max(MIN_AREA.w, area.w + dx), h: Math.max(MIN_AREA.h, area.h + dy) }),
      () => {
        if (size) onresize(Math.round(size.w), Math.round(size.h));
        size = null;
      },
    );
</script>

<div
  class="area"
  style:--c={color}
  style:left="{area.x}px"
  style:top="{area.y}px"
  style:width="{size?.w ?? area.w}px"
  style:height="{size?.h ?? area.h}px"
  role="group"
  aria-label="{name} area"
  use:paperMenu={menu}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="name" title="Drag to move the area and its cards · right-click for options" onpointerdown={dragName}>{name}</span>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="corner" title="Drag to resize" onpointerdown={dragCorner}></span>
</div>

<style>
  .area {
    position: absolute;
    border-radius: 14px;
    background: color-mix(in srgb, var(--c) 9%, transparent);
    border: 1.5px solid color-mix(in srgb, var(--c) 32%, transparent);
    transition:
      left calc(0.55s * var(--m)) var(--ease),
      top calc(0.55s * var(--m)) var(--ease);
  }

  .name {
    position: absolute;
    left: 12px;
    top: -12px;
    padding: 3px 10px;
    border-radius: 8px;
    background: var(--c);
    color: #fff;
    font: 700 11px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: grab;
    user-select: none;
    touch-action: none;
    box-shadow: var(--shadow);
    max-width: calc(100% - 24px);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .name:active {
    cursor: grabbing;
  }

  .corner {
    position: absolute;
    right: 2px;
    bottom: 2px;
    width: 16px;
    height: 16px;
    cursor: nwse-resize;
    touch-action: none;
    border-right: 3px solid color-mix(in srgb, var(--c) 55%, transparent);
    border-bottom: 3px solid color-mix(in srgb, var(--c) 55%, transparent);
    border-radius: 0 0 10px 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .area {
      transition: none;
    }
  }
</style>
