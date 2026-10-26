<script lang="ts">
  // A box on the Desk: a project's area (cards dropped inside get the project) or a plain
  // labelled box that only groups cards (round 12). Dragging the name moves the box and the
  // cards in it; the corner resizes it; ▴ folds it up to its name and a count whose colour
  // says how its cards stand (grey: nothing waiting, blue: something from today, orange:
  // something still open or brought up). Double-click a label to rename it. Right-click for
  // its paper menu.
  import { focusNow } from '../lib/focus';
  import { MIN_AREA, type DeskBox } from '../lib/canvasView';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { projectById, projectColor } from '../lib/store.svelte';
  import { dragButton } from '../lib/rightDrag';
  import Icon from './Icon.svelte';

  let {
    box,
    zoom,
    onmove,
    onresize,
    ontoggle,
    onrename,
    menu,
    renaming = $bindable(false),
  }: {
    box: DeskBox;
    zoom: number;
    /** Moving (live, `done` false) or dropped (`done` true), by this much in canvas units. */
    onmove: (dx: number, dy: number, done: boolean) => void;
    /** Resized to this size (on letting go). */
    onresize: (w: number, h: number) => void;
    /** Fold up or open out. */
    ontoggle: () => void;
    /** A labelled box got a new name. */
    onrename: (label: string) => void;
    menu: () => MenuSpec;
    /** The label is being edited. */
    renaming?: boolean;
  } = $props();

  let size = $state<{ w: number; h: number } | null>(null);
  let draft = $state('');

  const name = $derived(box.projectId ? (projectById(box.projectId)?.name ?? 'Project') : box.label);
  const color = $derived(box.projectId ? projectColor(box.projectId) : 'var(--mut)');
  const what = $derived(
    box.status === 'waiting' ? 'something still open or brought up' : box.status === 'today' ? 'something from today' : 'nothing waiting',
  );

  $effect(() => {
    if (renaming) draft = box.label;
  });

  function track(e: PointerEvent, step: (dx: number, dy: number) => void, end: (dx: number, dy: number) => void) {
    // Buttons and the rename box on the name tab get their own clicks (the Desk mustn't pan).
    if ((e.target as HTMLElement).closest('button, input')) {
      e.stopPropagation();
      return;
    }
    if (!dragButton(e) || renaming) return;
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
      (dx, dy) => (size = { w: Math.max(MIN_AREA.w, box.w + dx), h: Math.max(MIN_AREA.h, box.h + dy) }),
      () => {
        if (size) onresize(Math.round(size.w), Math.round(size.h));
        size = null;
      },
    );

  function finishRename(keep: boolean) {
    renaming = false;
    if (keep && draft.trim() && draft.trim() !== box.label) onrename(draft.trim());
  }
</script>

<div
  class="area"
  class:label={!box.projectId}
  class:collapsed={box.collapsed}
  style:--c={color}
  style:left="{box.x}px"
  style:top="{box.y}px"
  style:width={box.collapsed ? 'auto' : `${size?.w ?? box.w}px`}
  style:height={box.collapsed ? 'auto' : `${size?.h ?? box.h}px`}
  role="group"
  aria-label="{name} box"
  use:paperMenu={menu}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="name"
    title="Drag to move the box and its cards · right-click for options"
    onpointerdown={dragName}
    ondblclick={(e) => {
      e.stopPropagation();
      if (!box.projectId) renaming = true;
    }}
  >
    {#if renaming}
      <input
        class="rename"
        bind:value={draft}
        use:focusNow
        maxlength="60"
        aria-label="Box name"
        onpointerdown={(e) => e.stopPropagation()}
        onblur={() => finishRename(true)}
        onkeydown={(e) => {
          if (e.key === 'Enter') finishRename(true);
          else if (e.key === 'Escape') {
            e.stopPropagation();
            finishRename(false);
          }
        }}
      />
    {:else}
      <span class="txt">{name}</span>
    {/if}
    {#if box.collapsed || box.count > 0}
      <span class="count {box.status}" title="{box.count} card{box.count === 1 ? '' : 's'}, {what}">{box.count}</span>
    {/if}
    <button
      type="button"
      class="fold"
      title={box.collapsed ? 'Open it out' : 'Fold it up'}
      aria-label={box.collapsed ? `Open ${name}` : `Fold up ${name}`}
      aria-expanded={!box.collapsed}
      onclick={(e) => {
        e.stopPropagation();
        ontoggle();
      }}
    >
      <Icon name={box.collapsed ? 'down' : 'up'} size={12} />
    </button>
  </span>
  {#if !box.collapsed}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="corner" title="Drag to resize" onpointerdown={dragCorner}></span>
  {/if}
</div>

<style>
  .area {
    position: absolute;
    border-radius: 14px;
    background: color-mix(in srgb, var(--c) 9%, transparent);
    border: 1.5px solid color-mix(in srgb, var(--c) 32%, transparent);
    transition:
      left calc(0.55s * var(--m)) var(--ease),
      top calc(0.55s * var(--m)) var(--ease),
      width calc(0.3s * var(--m)) var(--ease),
      height calc(0.3s * var(--m)) var(--ease);
  }

  .area.label {
    border-style: dashed;
    background: color-mix(in srgb, var(--ink) 4%, transparent);
  }

  .area.collapsed {
    background: none;
    border: 0;
  }

  .name {
    position: absolute;
    left: 12px;
    top: -12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 4px 3px 10px;
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
    max-width: max(calc(100% - 24px), 160px);
    white-space: nowrap;
  }

  .collapsed .name {
    position: relative;
    left: 0;
    top: 0;
    max-width: 320px;
    padding: 6px 6px 6px 12px;
    font-size: 12px;
    box-shadow: var(--lift);
  }

  .txt {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .name:active {
    cursor: grabbing;
  }

  .count {
    min-width: 20px;
    padding: 0 6px;
    border-radius: 9px;
    text-align: center;
    font: 700 11px/18px var(--ui);
    letter-spacing: 0;
    color: #fff;
    background: #8a919a;
    box-shadow: 0 0 0 1.5px rgba(255, 255, 255, 0.85);
  }

  .count.today {
    background: var(--accent);
  }

  .count.waiting {
    background: #e07b1a;
  }

  .fold {
    width: 20px;
    height: 20px;
    border: 0;
    padding: 0;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .fold:hover {
    background: rgba(255, 255, 255, 0.34);
  }

  .rename {
    font: 700 11px var(--ui);
    letter-spacing: 0.04em;
    width: 160px;
    border: 0;
    border-radius: 5px;
    padding: 1px 5px;
    color: var(--ink);
    background: var(--paper);
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
