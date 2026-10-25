// One toast at a time at the bottom of the window, with an optional Undo-style button.
// A toast with a button stays 10 seconds, and waits while the pointer or focus is on it.
// Round 15: each Undo toast also goes in the undo history (lib/undoHistory.ts), which
// Ctrl+Z and "undo" in Find use.

import { remember, undoFrom, type Change } from './undoHistory';

export const toastState = $state({ text: '', action: '', on: false, serial: 0 });

/** How long a plain toast stays. */
export const PLAIN_MS = 4800;
/** How long a toast with a button (Undo) stays. */
export const ACTION_MS = 10_000;
/** Time left after the pointer or focus leaves a toast, at least. */
const AFTER_HOLD_MS = 3000;

let handler: (() => void) | null = null;
/** The history entry the toast's Undo button belongs to. */
let handlerChange: number | null = null;
let nextChange = 1;
/** The last change undone, for Ctrl+Shift+Z. */
let lastUndone: Change | null = null;

/** Changes that can be undone, newest last. */
export const changes = $state<{ list: Change[] }>({ list: [] });
let timer: ReturnType<typeof setTimeout> | null = null;
let endsAt = 0;
let held = false;

function schedule(ms: number): void {
  if (timer !== null) clearTimeout(timer);
  endsAt = Date.now() + ms;
  timer = setTimeout(() => {
    timer = null;
    toastState.on = false;
    // A hidden toast's Undo must not run later (it stays in the page while it fades).
    handler = null;
  }, ms);
}

/**
 * Show `text`. With `run` it gets a button named `action`; an "Undo" button also goes in the
 * undo history, and `redo` (where the caller knows how) lets it be done again after an undo.
 */
export function toast(
  text: string,
  action = '',
  run: (() => void) | null = null,
  redo: (() => void) | null = null,
): void {
  show(text, run ? action : '', run);
  if (run && action === 'Undo') {
    const change: Change = { id: nextChange++, text, at: Date.now(), undo: run, redo };
    changes.list = remember(changes.list, change);
    handlerChange = change.id;
  }
}

function show(text: string, action: string, run: (() => void) | null): void {
  toastState.text = text;
  toastState.action = action;
  toastState.on = true;
  toastState.serial += 1;
  handler = run;
  handlerChange = null;
  held = false;
  schedule(run ? ACTION_MS : PLAIN_MS);
}

/** The pointer or focus is on the toast: stop the clock. */
export function holdToast(): void {
  if (!toastState.on || held) return;
  held = true;
  if (timer !== null) clearTimeout(timer);
  timer = null;
}

/** The pointer or focus left: carry on with what was left, but at least a few seconds. */
export function releaseToast(): void {
  if (!toastState.on || !held) return;
  held = false;
  schedule(Math.max(endsAt - Date.now(), AFTER_HOLD_MS));
}

export function runToastAction(): void {
  const run = handler;
  const change = handlerChange;
  handler = null;
  handlerChange = null;
  held = false;
  toastState.on = false;
  if (change !== null) undoBack(change, false);
  else run?.();
}

/**
 * Undo the change `id` and every change after it, newest first. With `say`, a toast says
 * what was undone and offers Redo when there was one change that knows how.
 */
export function undoBack(id: number, say = true): void {
  const { kept, undo } = undoFrom(changes.list, id);
  if (!undo.length) return;
  changes.list = kept;
  for (const c of undo) c.undo();
  const last = undo[undo.length - 1]!;
  lastUndone = undo.length === 1 ? last : null;
  if (!say) return;
  const more = kept.length ? ' · Ctrl+Z again for the one before' : '';
  const what = undo.length === 1 ? last.text : `${undo.length} changes, back to “${last.text}”`;
  const redo = lastUndone?.redo ? redoLast : null;
  show(`Undone: ${what}${more}`, redo ? 'Redo' : '', redo);
}

/** Ctrl+Z: undo the newest change. False when there is nothing to undo. */
export function undoLatest(): boolean {
  const last = changes.list[changes.list.length - 1];
  if (!last) return false;
  undoBack(last.id);
  return true;
}

/** Redo (the toast button or Ctrl+Shift+Z): do the last undone change again. */
export function redoLast(): boolean {
  const c = lastUndone;
  if (!c?.redo) return false;
  lastUndone = null;
  c.redo();
  toast(c.text, 'Undo', c.undo, c.redo);
  return true;
}
