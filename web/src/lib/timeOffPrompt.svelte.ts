// "Book time off…" (round 21): a box asking for the first and last day off, and the two
// ways of booking (a range from the box, one day straight from a menu).
import { api } from './api';
import { refreshBell } from './bell.svelte';
import { attempt, refresh } from './store.svelte';
import { dayOfMonth, weekdayShort } from './format';
import { offDone } from './timeOff';
import { toast } from './toast.svelte';
import type { TimeOffBooked } from './api';

export const timeOffPrompt = $state({ open: false, from: '', serial: 0 });

/** Open the box with `from` as the first (and, to begin with, last) day. */
export function askTimeOff(from: string): void {
  timeOffPrompt.from = from;
  timeOffPrompt.open = true;
  timeOffPrompt.serial += 1;
}

/** Close the box. */
export function closeTimeOff(): void {
  timeOffPrompt.open = false;
}

async function after(): Promise<void> {
  await refresh();
  await refreshBell();
}

/** One toast for the lot, with an Undo that takes every day back. */
function booked(done: TimeOffBooked, dates: string[]): void {
  const each = (act: (id: string) => Promise<unknown>) => async () => {
    for (const id of done.entries) await attempt(() => act(id));
    await after();
  };
  toast(
    offDone(dates, done.hours),
    'Undo',
    () => void each((id) => api.deleteEntry(id))(),
    () => void each((id) => api.restoreEntry(id))(),
  );
}

/** Book one day off, whatever is on it. Returns the new entry's id. */
export async function bookDayOff(date: string): Promise<string | null> {
  const done = await attempt(() => api.bookDayOff(date));
  if (!done) return null;
  await after();
  booked(done, [date]);
  return done.entries[0] ?? null;
}

/** Book the work days from `from` to `to` that have nothing logged. */
export async function bookTimeOff(from: string, to: string, dates: string[]): Promise<boolean> {
  const done = await attempt(() => api.bookTimeOff(from, to));
  if (!done) return false;
  await after();
  booked(done, dates);
  return true;
}

/** Take a day off back: its "Holiday" entries go, with an Undo. */
export async function unbookDayOff(date: string): Promise<void> {
  const ids = await attempt(() => api.unbookDayOff(date));
  if (!ids) return;
  await after();
  if (ids.length === 0) return;
  const each = (act: (id: string) => Promise<unknown>) => async () => {
    for (const id of ids) await attempt(() => act(id));
    await after();
  };
  toast(
    `${weekdayShort(date)} ${dayOfMonth(date)} is no longer a day off`,
    'Undo',
    () => void each((id) => api.restoreEntry(id))(),
    () => void each((id) => api.deleteEntry(id))(),
  );
}
