<script lang="ts">
  // A pile of cards on the Desk (round 8): drop a card on a card to make one. It shows its top
  // card with the rest as sheets behind and a count. Click it to fan the cards out (drag one
  // away to take it off); drag the pile to move it, or onto a drawer to file it all.
  // Double-click the name to rename it. Right-click for its paper menu. The Done today pile
  // (round 14) gathers the tickets ticked done today; it moves and fans out but keeps its name.
  import type { CanvasCard, CanvasStack } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { KIND_LABEL } from '../lib/notes';
  import { dragButton } from '../lib/rightDrag';

  let {
    pile,
    top,
    zoom,
    fanned,
    dropping = false,
    onmove,
    ondrag,
    ontoggle,
    onrename,
    menu,
    renaming = $bindable(false),
  }: {
    pile: CanvasStack;
    /** The card on top, shown as the pile's face. */
    top: CanvasCard;
    zoom: number;
    /** Its cards are spread out beside it. */
    fanned: boolean;
    /** Something is being dragged onto it. */
    dropping?: boolean;
    /** Dropped at a new spot (canvas units), with the pointer's screen point. */
    onmove: (x: number, y: number, at: { cx: number; cy: number }) => void;
    /** Being dragged, with the pointer's screen point. */
    ondrag: (at: { cx: number; cy: number }) => void;
    ontoggle: () => void;
    onrename: (name: string) => void;
    menu: () => MenuSpec;
    renaming?: boolean;
  } = $props();

  let drag = $state<{ dx: number; dy: number } | null>(null);
  let draft = $state('');
  let el: HTMLElement | undefined = $state();

  const label = $derived(pile.name ?? `${pile.count} cards`);
  const face = $derived(
    top.entry
      ? { kind: 'Entry', title: top.entry.description || 'Untitled' }
      : top.tray
        ? { kind: 'Ticket', title: top.tray.item.ticket }
        : top.note
          ? { kind: `${KIND_LABEL[top.note.kind]} note`, title: top.note.title }
          : { kind: 'Inbox scrap', title: top.inbox?.text ?? '' },
  );

  $effect(() => {
    if (renaming) draft = pile.name ?? '';
  });

  function down(e: PointerEvent) {
    if (!dragButton(e) || renaming) return;
    if ((e.target as HTMLElement).closest('input, button')) return;
    e.stopPropagation();
    // No text selection or browser drag from a pile (see CanvasCard).
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    el?.focus({ preventScroll: true });
    const start = { x: e.clientX, y: e.clientY };
    let moved = false;
    el?.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      if (!moved && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 4) return;
      moved = true;
      drag = { dx: (m.clientX - start.x) / zoom, dy: (m.clientY - start.y) / zoom };
      ondrag({ cx: m.clientX, cy: m.clientY });
    };
    const up = (u: PointerEvent) => {
      el?.removeEventListener('pointermove', move);
      el?.removeEventListener('pointerup', up);
      el?.removeEventListener('pointercancel', up);
      if (moved && drag) onmove(Math.round(pile.x + drag.dx), Math.round(pile.y + drag.dy), { cx: u.clientX, cy: u.clientY });
      // A right-click that didn't move is for the menu, not for fanning the pile.
      else if (e.button === 0) ontoggle();
      drag = null;
    };
    el?.addEventListener('pointermove', move);
    el?.addEventListener('pointerup', up);
    el?.addEventListener('pointercancel', up);
  }

  function finish(keep: boolean) {
    renaming = false;
    if (keep && draft.trim() !== (pile.name ?? '')) onrename(draft.trim());
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  data-pile={pile.id}
  class="pile"
  class:lifted={!!drag}
  class:fanned
  class:dropping
  class:done={pile.done}
  bind:this={el}
  style:left="{pile.x + (drag?.dx ?? 0)}px"
  style:top="{pile.y + (drag?.dy ?? 0)}px"
  style:z-index={drag ? 99999 : 50000}
  role="button"
  tabindex="0"
  aria-expanded={fanned}
  aria-label="{label}, a pile of {pile.count} cards"
  title={pile.done
    ? "Tickets you ticked done today. Click to fan out · drag to move · they leave the Desk at the end of the day"
    : 'Click to fan out · drag to move or file · right-click for options'}
  onpointerdown={down}
  ondblclick={(e) => e.stopPropagation()}
  onkeydown={(e) => {
    if (e.target === el && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      ontoggle();
    }
  }}
  use:paperMenu={menu}
>
  <span class="sheet s2" aria-hidden="true"></span>
  <span class="sheet s1" aria-hidden="true"></span>
  <div class="face paper">
    <span class="kind">{face.kind}</span>
    <span class="title">{face.title}</span>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="tab"
    ondblclick={(e) => {
      e.stopPropagation();
      renaming = true;
    }}
  >
    {#if renaming}
      <input
        class="rename"
        bind:value={draft}
        use:focusNow
        maxlength="60"
        placeholder="Name this pile"
        aria-label="Pile name"
        onpointerdown={(e) => e.stopPropagation()}
        onblur={() => finish(true)}
        onkeydown={(e) => {
          if (e.key === 'Enter') finish(true);
          else if (e.key === 'Escape') {
            e.stopPropagation();
            finish(false);
          }
        }}
      />
    {:else}
      {#if pile.name}<span class="nm">{pile.name}</span>{/if}
      <span class="count">{pile.count}</span>
    {/if}
  </span>
</div>

<style>
  .pile {
    position: absolute;
    width: 220px;
    height: 96px;
    cursor: grab;
    user-select: none;
    touch-action: none;
    transition:
      left calc(0.45s * var(--m)) var(--ease),
      top calc(0.45s * var(--m)) var(--ease);
  }

  .pile.lifted {
    cursor: grabbing;
    transition: none;
  }

  .pile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 4px;
    border-radius: 6px;
  }

  .sheet {
    position: absolute;
    inset: 0;
    border-radius: 5px;
    background: var(--paper);
    box-shadow: var(--shadow);
    transition: transform calc(0.3s * var(--m)) var(--spring);
  }

  .s1 {
    transform: translate(5px, 5px) rotate(1.2deg);
  }

  .s2 {
    transform: translate(10px, 10px) rotate(-1deg);
  }

  .pile:hover .s1 {
    transform: translate(7px, 7px) rotate(2deg);
  }

  .pile:hover .s2 {
    transform: translate(14px, 13px) rotate(-1.6deg);
  }

  .pile.fanned .sheet {
    transform: none;
    opacity: 0;
  }

  .face {
    position: absolute;
    inset: 0;
    border-radius: 5px;
    padding: 9px 11px;
    border-top: 3px solid var(--ink-2);
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .pile.lifted .face {
    box-shadow: var(--lift);
  }

  .pile.dropping .face,
  .pile.fanned .face {
    box-shadow:
      0 0 0 2px var(--accent),
      var(--shadow);
  }

  .kind {
    font: 700 10px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
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

  .tab {
    position: absolute;
    right: -8px;
    top: -10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px 2px 8px;
    border-radius: 9px;
    background: var(--ink);
    color: var(--paper);
    font: 700 11px var(--ui);
    box-shadow: var(--shadow);
    max-width: 200px;
  }

  .pile.done .face {
    border-top-color: var(--ok);
  }

  .pile.done .tab {
    background: var(--ok);
  }

  .tab:not(:has(.nm)) {
    padding-left: 4px;
  }

  .nm {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    min-width: 18px;
    text-align: center;
    border-radius: 8px;
    padding: 0 5px;
    background: color-mix(in srgb, var(--paper) 22%, transparent);
  }

  .rename {
    font: 600 11.5px var(--ui);
    width: 150px;
    border: 0;
    border-radius: 5px;
    padding: 1px 5px;
    color: var(--ink);
    background: var(--paper);
  }

  @media (prefers-reduced-motion: reduce) {
    .pile,
    .sheet {
      transition: none;
    }
  }
</style>
