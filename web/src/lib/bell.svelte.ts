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

/** Minutes since local midnight. */
export function nowMinutes(at = new Date()): number {
  return at.getHours() * 60 + at.getMinutes();
}

/** Ask what's coming up now. Quiet on failure: the bell just keeps what it had. */
export async function refreshBell(): Promise<void> {
  try {
    bell.items = await api.bell(app.today, nowMinutes());
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
