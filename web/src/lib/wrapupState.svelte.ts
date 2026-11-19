// Whether today's wrap-up is waiting, shared by the card, the Desk's panel (which puts it on
// top and folds the rest) and the bell's dot (round 21).

import { app, savePref } from './store.svelte';
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
  return !!at && !wrapupClosedToday() && (app.wrapupNow === app.today || isWrapupTime(clock.now, at));
}

/** True once today's wrap-up was finished or put off, here or before a lock or restart. */
export function wrapupClosedToday(): boolean {
  return app.wrapupClosed === app.today || (app.wrapupClosed !== REOPENED && app.prefs?.wrapupClosed === app.today);
}

// "Wrap up the day" was asked for again after it was closed today.
const REOPENED = 'reopened';

/** The wrap-up is finished or put off for today. It stays away through a lock or a restart. */
export function closeWrapup(): void {
  app.wrapupClosed = app.today;
  void savePref('day.wrapup_closed', app.today);
}

/** Bring today's wrap-up back (Undo of "put away", or "Wrap up the day" in Find). */
export function reopenWrapup(): void {
  app.wrapupClosed = REOPENED;
  if (app.prefs?.wrapupClosed) void savePref('day.wrapup_closed', '');
}
