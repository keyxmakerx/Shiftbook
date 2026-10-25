// Windows (round 12): a notepad or note popped out to float over Shiftbook. Each is open
// (floating), rolled up to its title bar, minimised to the bottom bar or full screen. The
// list is saved as one preference (`ui.windows`, kept in bounds by
// crates/core/src/windows.rs) so windows come back where they were.
import { api } from './api';
import type { CardKind, WindowMode, WindowSpot } from './api';
import { app } from './store.svelte';
import { phone } from './phone.svelte';

/** Most windows at once; opening one more closes the oldest (same as the core rule). */
const MAX_WINDOWS = 12;
const SIZE = { w: 420, h: 320 };
const SAVE_AFTER_MS = 600;

export const windows = $state<{
  list: WindowSpot[];
  front: string | null;
  loaded: boolean;
  /** Each window's title once it has loaded what it shows. */
  titles: Record<string, string>;
}>({ list: [], front: null, loaded: false, titles: {} });

export const winKey = (w: { kind: CardKind; id: string }) => `${w.kind}:${w.id}`;

/** The stored form: `kind:id mode x y w h`, `;` between windows. */
function format(list: WindowSpot[]): string {
  return list.map((w) => `${w.kind}:${w.id} ${w.mode} ${Math.round(w.x)} ${Math.round(w.y)} ${Math.round(w.w)} ${Math.round(w.h)}`).join(';');
}

/** The window drawn on top that isn't minimised, if any. */
function topKey(): string | null {
  for (let i = windows.list.length - 1; i >= 0; i--) {
    const w = windows.list[i]!;
    if (w.mode !== 'min') return winKey(w);
  }
  return null;
}

let saving: ReturnType<typeof setTimeout> | null = null;

function save() {
  if (saving) clearTimeout(saving);
  saving = setTimeout(() => {
    saving = null;
    // Saved quietly: a window that can't be remembered still works for now.
    void api.savePref('ui.windows', format(windows.list)).catch(() => undefined);
  }, SAVE_AFTER_MS);
}

/** Take the windows saved last time (after unlock). */
export function loadWindows(): void {
  windows.list = (app.prefs?.windows ?? []).map((w) => ({ ...w }));
  windows.front = topKey();
  windows.loaded = true;
}

function find(key: string): WindowSpot | undefined {
  return windows.list.find((w) => winKey(w) === key);
}

/** Bring a window to the front (last in the list draws on top). */
export function focusWindow(key: string): void {
  const i = windows.list.findIndex((w) => winKey(w) === key);
  if (i < 0) return;
  const [w] = windows.list.splice(i, 1);
  windows.list.push(w!);
  windows.front = key;
  save();
}

/** Open a window for a record, or bring its window back if it already has one. */
export function openWindow(kind: CardKind, id: string): void {
  const key = `${kind}:${id}`;
  const there = find(key);
  if (there) {
    // A phone has room for one window at a time: it opens full screen.
    if (phone.on) there.mode = 'full';
    else if (there.mode === 'min' || there.mode === 'rolled') there.mode = 'open';
    focusWindow(key);
    return;
  }
  // Cascade new windows from the top left of the work area.
  const n = windows.list.filter((w) => w.mode === 'open').length;
  const spot: WindowSpot = { kind, id, mode: phone.on ? 'full' : 'open', x: 80 + (n % 6) * 28, y: 40 + (n % 6) * 28, ...SIZE };
  windows.list.push(spot);
  while (windows.list.length > MAX_WINDOWS) windows.list.shift();
  windows.front = key;
  save();
}

export function setWindowMode(key: string, mode: WindowMode): void {
  const w = find(key);
  if (!w) return;
  w.mode = mode;
  if (mode === 'min') {
    windows.front = topKey();
    save();
  } else {
    focusWindow(key);
  }
}

export function moveWindow(key: string, x: number, y: number): void {
  const w = find(key);
  if (!w) return;
  w.x = x;
  w.y = Math.max(0, y);
  save();
}

export function resizeWindow(key: string, width: number, height: number): void {
  const w = find(key);
  if (!w) return;
  w.w = Math.max(260, width);
  w.h = Math.max(160, height);
  save();
}

export function closeWindow(key: string): void {
  windows.list = windows.list.filter((w) => winKey(w) !== key);
  if (windows.front === key) windows.front = topKey();
  save();
}

/** A click on a window's button in the bottom bar: show it, or tuck it away if it's in front. */
export function toggleFromBar(key: string): void {
  const w = find(key);
  if (!w) return;
  if (w.mode === 'min') setWindowMode(key, 'open');
  else if (windows.front === key) setWindowMode(key, 'min');
  else focusWindow(key);
}
