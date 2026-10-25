// The bell (round 12): what's coming up, from crates/core/src/bell.rs. Checked every minute
// while unlocked and whenever the day's data changes; the panel slides in from the right.
import { api } from './api';
import type { BellItem } from './api';
import { app } from './store.svelte';

export const bell = $state<{ items: BellItem[]; open: boolean; away: { since: string | null; items: BellItem[] } | null }>({
  items: [],
  open: false,
  away: null,
});

/**
 * "Remind me at 16:00" on the timesheet reminder: hidden until then (minutes after midnight)
 * on that day. Kept only until the app locks.
 */
let sheetSnooze: { day: string; until: number } | null = null;

/** The hour "Remind me at…" offers: two hours on, on the hour, at the latest 23:00. */
export function snoozeHour(now: number): number {
  return Math.min(23, Math.floor(now / 60) + 2) * 60;
}

export function snoozeSheet(): void {
  sheetSnooze = { day: app.today, until: snoozeHour(nowMinutes()) };
  bell.items = bell.items.filter((i) => i.about.kind !== 'timesheet');
}

/** Minutes since local midnight. */
export function nowMinutes(at = new Date()): number {
  return at.getHours() * 60 + at.getMinutes();
}

/** Ask what's coming up now. Quiet on failure: the bell just keeps what it had. */
export async function refreshBell(): Promise<void> {
  try {
    const now = nowMinutes();
    const items = await api.bell(app.today, now);
    const snoozed = sheetSnooze?.day === app.today && now < sheetSnooze.until;
    bell.items = snoozed ? items.filter((i) => i.about.kind !== 'timesheet') : items;
  } catch {
    // The next tick tries again.
  }
}

/**
 * "While you were away" (round 15): after unlock, or when the day changes, open the panel on
 * what came due since Shiftbook was last open, if anything did. Quiet on failure.
 */
export async function openAway(): Promise<void> {
  try {
    const away = await api.whileAway(app.today, nowMinutes());
    if (away.items.length === 0) return;
    bell.away = away;
    bell.open = true;
  } catch {
    // Nothing to show; the bell still has everything.
  }
}
