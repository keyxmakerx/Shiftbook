// Auto-lock timing, kept pure so it can be tested without a browser.
//
// Two reasons to lock:
// - idle: no keyboard or mouse activity for `idleMs` (15 minutes by default);
// - asleep: the page's timer missed a long stretch, which happens when the computer
//   sleeps or hibernates (timers stop while suspended). This is a stand-in for real
//   OS sleep/lock events, which the app can't see yet.

export const DEFAULT_IDLE_MS = 15 * 60 * 1000;
/** How often the page checks. */
export const TICK_MS = 10 * 1000;
/** A gap this long between checks means the computer was probably asleep. */
export const SLEEP_GAP_MS = 60 * 1000;

export type LockReason = 'idle' | 'asleep';

export interface IdleState {
  lastActivity: number;
  lastTick: number;
}

export function startIdle(now: number): IdleState {
  return { lastActivity: now, lastTick: now };
}

/** Called on every keyboard/mouse event. */
export function recordActivity(state: IdleState, now: number): IdleState {
  return { ...state, lastActivity: now };
}

/** Called every TICK_MS. Returns the new state and, if it's time, why to lock. */
export function checkIdle(
  state: IdleState,
  now: number,
  idleMs: number = DEFAULT_IDLE_MS,
): { state: IdleState; lock: LockReason | null } {
  const next = { ...state, lastTick: now };
  if (now - state.lastTick >= SLEEP_GAP_MS) return { state: next, lock: 'asleep' };
  if (now - state.lastActivity >= idleMs) return { state: next, lock: 'idle' };
  return { state: next, lock: null };
}
