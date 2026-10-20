// Pins (round 14): entries, tickets and notes kept in view as chips on the bottom bar, each
// with an optional day to chase it (rules in crates/core/src/pins.rs). Loaded after unlock,
// each minute with the bell, and after every change.
import { api } from './api';
import type { CardKind, PinItem } from './api';
import { refreshBell } from './bell.svelte';
import { addDays } from './format';
import type { MenuItem, MenuSpec } from './menu.svelte';
import { app, attempt } from './store.svelte';
import { toast } from './toast.svelte';
import { openWindow } from './windows.svelte';

export const pins = $state<{ list: PinItem[] }>({ list: [] });

const find = (kind: CardKind, id: string) => pins.list.find((p) => p.kind === kind && p.id === id);

/** Ask for the chips. Quiet on failure: the bar keeps what it had. */
export async function refreshPins(): Promise<void> {
  try {
    pins.list = await api.pins(app.today);
  } catch {
    // The next refresh tries again.
  }
}

async function changed() {
  await refreshPins();
  await refreshBell();
}

/** Pin something to the bottom bar (Undo takes it off again). */
export async function pin(kind: CardKind, id: string): Promise<void> {
  if (!(await attempt(() => api.pin(kind, id, null).then(() => true)))) return;
  await changed();
  toast('Pinned to the bottom bar. Right-click its chip to set a day to chase it.', 'Undo', () => void unpin(kind, id, false));
}

/** Take a chip off the bar, with Undo (which keeps its chase date). */
export async function unpin(kind: CardKind, id: string, undoable = true): Promise<void> {
  const was = find(kind, id);
  if (!(await attempt(() => api.unpin(kind, id).then(() => true)))) return;
  await changed();
  if (undoable) {
    toast('Unpinned', 'Undo', () => void attempt(() => api.pin(kind, id, was?.chaseOn ?? null)).then(changed));
  }
}

/** Set (or clear, with null) the day to chase a pinned thing. */
export async function chaseOn(kind: CardKind, id: string, date: string | null): Promise<void> {
  if (await attempt(() => api.pin(kind, id, date).then(() => true))) await changed();
}

/** "Pin to the bottom bar" or "Unpin", for a record's right-click menu. */
export function pinMenuItem(kind: CardKind, id: string): MenuItem {
  return find(kind, id)
    ? { label: 'Unpin from the bottom bar', icon: 'pin', side: true, act: () => void unpin(kind, id) }
    : { label: 'Pin to the bottom bar', icon: 'pin', side: true, act: () => void pin(kind, id) };
}

/** A chip's own menu. */
export function chipMenu(p: PinItem): MenuSpec {
  const day = (n: number) => addDays(app.today, n);
  return {
    sections: [
      {
        head: p.title,
        items: [
          { label: 'Open', icon: 'open', side: true, act: () => openWindow(p.kind, p.id) },
          { label: 'Unpin', icon: 'x', side: true, act: () => void unpin(p.kind, p.id) },
        ],
      },
      {
        head: 'Chase it',
        choices: [
          { label: 'Today', color: null, checked: p.chaseOn === app.today, act: () => void chaseOn(p.kind, p.id, app.today) },
          { label: 'Tomorrow', color: null, checked: p.chaseOn === day(1), act: () => void chaseOn(p.kind, p.id, day(1)) },
          { label: 'In 3 days', color: null, checked: p.chaseOn === day(3), act: () => void chaseOn(p.kind, p.id, day(3)) },
          { label: 'Next week', color: null, checked: p.chaseOn === day(7), act: () => void chaseOn(p.kind, p.id, day(7)) },
          { label: 'No chase date', color: null, checked: p.chaseOn === null, act: () => void chaseOn(p.kind, p.id, null) },
        ],
      },
    ],
  };
}
