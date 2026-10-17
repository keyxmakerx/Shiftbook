// Counting for the optional timer, kept pure so it can be tested without a browser.
// What the counted time becomes (rounding, leaving out time away) is decided by the app
// (crates/core/src/timer.rs); this only counts.

/** A gap this long between ticks means the computer slept: that time isn't counted. */
export const SLEEP_GAP_MS = 60 * 1000;
/** A quiet stretch shorter than this isn't worth reporting. */
export const QUIET_PROBE_MS = 60 * 1000;

export interface Clock {
  /** Milliseconds counted. */
  counted: number;
  lastTick: number;
  lastActivity: number;
  /** Quiet stretches noticed, in whole seconds. */
  quiet: number[];
}

export function startClock(now: number, counted = 0, quiet: number[] = []): Clock {
  return { counted, lastTick: now, lastActivity: now, quiet: [...quiet] };
}

/** Called about once a second while running. A long gap (sleep) is skipped, not counted. */
export function tickClock(c: Clock, now: number): Clock {
  const gap = now - c.lastTick;
  const counted = gap > 0 && gap < SLEEP_GAP_MS ? c.counted + gap : c.counted;
  return { ...c, counted, lastTick: now };
}

/** Called on keyboard or mouse activity while running: notes a quiet stretch that just ended. */
export function noticeActivity(c: Clock, now: number): Clock {
  const gap = now - c.lastActivity;
  const quiet = gap >= QUIET_PROBE_MS ? [...c.quiet, Math.floor(gap / 1000)] : c.quiet;
  return { ...c, quiet, lastActivity: now };
}

/** Whole seconds counted. */
export function seconds(c: Clock): number {
  return Math.floor(c.counted / 1000);
}

/** "0:07", "12:05", "1:02:09". */
export function clockText(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}
