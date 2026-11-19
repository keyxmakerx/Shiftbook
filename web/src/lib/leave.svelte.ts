// The wrap-up offered on the way out (round 21): locking by hand, or closing the window,
// after the wrap-up time asks once a day whether to wrap up first. Whether it is due is the
// app's rule (`shiftbook_core::nudge::leave_offer`); this shows the box and does the answer.
import { api } from './api';
import type { LeaveOffer } from './api';
import { nowMinutes } from './bell.svelte';
import { app, savePref, selectDay, setView } from './store.svelte';
import { closeWrapup, wrapupDue } from './wrapupState.svelte';

export const leave = $state<{ open: boolean; offer: LeaveOffer | null; how: 'lock' | 'close'; at: Date }>({
  open: false,
  offer: null,
  how: 'lock',
  at: new Date(),
});

// What "Just lock" or "Just close" does, and the save that has to land before it.
let go: (() => void) | null = null;
let noted: Promise<unknown> = Promise.resolve();

function ask(): Promise<LeaveOffer | null> {
  return api.leaveOffer(app.today, nowMinutes()).catch(() => null);
}

function show(offer: LeaveOffer, how: 'lock' | 'close', then: () => void): void {
  leave.offer = offer;
  leave.how = how;
  leave.at = new Date();
  leave.open = true;
  go = then;
  // Once a day: asked now, whatever the answer.
  noted = savePref('day.wrapup_offered', app.today);
}

/** Lock by hand (the Lock button, Ctrl+L, "Lock now"): offer the wrap-up first when due. */
export async function lockOrOffer(lock: () => void): Promise<void> {
  if (leave.open) return;
  const offer = await ask();
  if (offer) show(offer, 'lock', lock);
  else lock();
}

/** The answer: wrap up (stay), go (lock or close), or not today (put it off, then go). */
export async function answerLeave(answer: 'wrapup' | 'go' | 'not-today' | 'stay'): Promise<void> {
  const then = go;
  go = null;
  leave.open = false;
  if (answer === 'stay') return;
  if (answer === 'wrapup') {
    if (app.view !== 'desk' && app.view !== 'day') setView('desk');
    await selectDay(app.today);
    return;
  }
  if (answer === 'not-today') closeWrapup();
  // Locking reloads the page: let what was just saved land first.
  await noted;
  if (answer === 'not-today') await savePref('day.wrapup_closed', app.today);
  then?.();
}

/** Keep the window's close in step: it asks first only while an offer is due. Called once by
 * the shell. */
export function watchLeave(): void {
  let armed = false;
  const arm = (on: boolean) => {
    if (on === armed) return;
    armed = on;
    void api.setLeaveGuard(on).catch(() => undefined);
  };

  $effect(() => {
    // The time has come, it isn't closed, and it wasn't offered yet today; anything logged
    // since (app.changed) can make it due.
    void app.changed;
    const maybe = app.ready && wrapupDue() && app.prefs?.wrapupOffered !== app.today;
    if (!maybe) {
      arm(false);
      return;
    }
    let stale = false;
    void ask().then((offer) => {
      if (!stale) arm(offer !== null);
    });
    return () => {
      stale = true;
    };
  });

  $effect(() => {
    const stop = api.onLeaveAsked(() => {
      // The close used the guard up, so a second close always goes through.
      armed = false;
      void ask().then((offer) => {
        if (offer && !leave.open) show(offer, 'close', () => void api.closeWindow());
        else if (!leave.open) void api.closeWindow();
      });
    });
    return () => {
      stop();
      arm(false);
    };
  });
}
