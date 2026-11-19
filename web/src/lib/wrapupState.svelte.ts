// Whether today's wrap-up is waiting, shared by the card, the Desk's panel (which puts it on
// top and folds the rest) and the bell's dot (round 21).

import { app } from './store.svelte';
import { isWrapupTime } from './wrapup';

const clock = $state({ now: new Date() });
let ticking = false;

/** Keep the clock moving once something asks; half a minute is close enough for a wrap-up. */
function tick(): void {
  if (ticking || typeof window === 'undefined') return;
  ticking = true;
  setInterval(() => (clock.now = new Date()), 30_000);
}

/**
 * True while today's wrap-up is waiting: a time is set, it has come (or "Wrap up the day"
 * was asked for), and it hasn't been finished or put off today.
 */
export function wrapupDue(): boolean {
  tick();
  const at = app.prefs?.wrapupAt ?? null;
  return !!at && app.wrapupClosed !== app.today && (app.wrapupNow === app.today || isWrapupTime(clock.now, at));
}
