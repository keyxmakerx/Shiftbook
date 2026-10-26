// Drag with the right mouse button too (Settings → Look, round 15, off by default).
//
// A right press that moves becomes a drag; one that doesn't still opens the menu. Browsers send
// the context menu event at different times: Windows on release, macOS and Linux on press. So
// while a right press is held its menu is put off until release (and dropped if it turned into a
// drag), and a menu that arrives just after a drag is dropped too.

import { app } from './store.svelte';

/** How far the pointer moves before a press counts as a drag (as the Desk's cards do). */
export const DRAG_SLOP_PX = 4;
/** A menu arriving this soon after a right drag ends belongs to that drag. */
export const AFTER_DRAG_MS = 400;

/** True when the press can start a drag: the left button, or the right one when it's on. */
export function dragButton(e: { button: number }): boolean {
  return e.button === 0 || (e.button === 2 && !!app.prefs?.rightDrag);
}

/** The right press being watched, and what to do with its menu. Pure, so it can be tested. */
export class RightPress {
  private start: { x: number; y: number } | null = null;
  private moved = false;
  private held: unknown = null;
  private dragEnded = -Infinity;

  down(x: number, y: number): void {
    this.start = { x, y };
    this.moved = false;
    this.held = null;
  }

  move(x: number, y: number): void {
    if (this.start && !this.moved && Math.hypot(x - this.start.x, y - this.start.y) >= DRAG_SLOP_PX) this.moved = true;
  }

  /** A context menu event: true to stop it now. `menu` is kept to open on release if needed. */
  menu(menu: unknown, now: number): boolean {
    if (this.start) {
      this.held = menu;
      return true;
    }
    return now - this.dragEnded < AFTER_DRAG_MS;
  }

  /** The right button let go: the held menu to open now, if the press was a plain click. */
  up(now: number): unknown {
    const held = this.moved ? null : this.held;
    if (this.start && this.moved) this.dragEnded = now;
    this.start = null;
    this.held = null;
    return held;
  }

  cancel(): void {
    this.start = null;
    this.held = null;
  }
}

type Held = { target: EventTarget; init: MouseEventInit };

/** Watch the whole window while the setting is on. Returns the clean-up. */
export function watchRightDrag(win: Window): () => void {
  const press = new RightPress();
  let replaying = false;
  const opts = { capture: true };
  const onDown = (e: PointerEvent) => {
    if (e.button === 2) press.down(e.clientX, e.clientY);
  };
  const onMove = (e: PointerEvent) => press.move(e.clientX, e.clientY);
  const onUp = (e: PointerEvent) => {
    if (e.button !== 2) return;
    const held = press.up(e.timeStamp) as Held | null;
    if (!held) return;
    replaying = true;
    try {
      held.target.dispatchEvent(new MouseEvent('contextmenu', held.init));
    } finally {
      replaying = false;
    }
  };
  const onMenu = (e: MouseEvent) => {
    if (replaying || !e.target) return;
    const init: MouseEventInit = {
      bubbles: true,
      cancelable: true,
      composed: true,
      clientX: e.clientX,
      clientY: e.clientY,
      screenX: e.screenX,
      screenY: e.screenY,
      button: 2,
      shiftKey: e.shiftKey,
      ctrlKey: e.ctrlKey,
      altKey: e.altKey,
      metaKey: e.metaKey,
    };
    if (press.menu({ target: e.target, init } satisfies Held, e.timeStamp)) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  };
  const onCancel = () => press.cancel();
  win.addEventListener('pointerdown', onDown, opts);
  win.addEventListener('pointermove', onMove, opts);
  win.addEventListener('pointerup', onUp, opts);
  win.addEventListener('pointercancel', onCancel, opts);
  win.addEventListener('blur', onCancel);
  win.addEventListener('contextmenu', onMenu, opts);
  return () => {
    win.removeEventListener('pointerdown', onDown, opts);
    win.removeEventListener('pointermove', onMove, opts);
    win.removeEventListener('pointerup', onUp, opts);
    win.removeEventListener('pointercancel', onCancel, opts);
    win.removeEventListener('blur', onCancel);
    win.removeEventListener('contextmenu', onMenu, opts);
  };
}
