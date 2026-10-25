// Swiping an entry row on a phone (round 15, idea A): right adds 15 minutes, left shows the
// Timer and Delete buttons. Kept pure so it can be tested without a browser.

/** How far a finger must move sideways before the row follows it. */
export const SWIPE_START_PX = 12;
/** How far a swipe must go to count. */
export const SWIPE_DONE_PX = 80;
/** Furthest the row slides. */
export const SWIPE_MAX_PX = 120;

export type SwipeEnd = 'add' | 'reveal' | 'none';

/** Is this movement a sideways swipe (not a scroll)? Decided once, at the start. */
export function isSideways(dx: number, dy: number): boolean {
  return Math.abs(dx) >= SWIPE_START_PX && Math.abs(dx) > Math.abs(dy) * 1.5;
}

/** How far the row is drawn, following the finger but not past the limit. */
export function slide(dx: number): number {
  return Math.max(-SWIPE_MAX_PX, Math.min(SWIPE_MAX_PX, dx));
}

/** What a finished swipe does. */
export function swipeEnd(dx: number): SwipeEnd {
  if (dx >= SWIPE_DONE_PX) return 'add';
  if (dx <= -SWIPE_DONE_PX) return 'reveal';
  return 'none';
}
