import { describe, expect, it } from 'vitest';
import { isSideways, slide, swipeEnd, SWIPE_DONE_PX, SWIPE_MAX_PX } from './swipe';

describe('swipe', () => {
  it('tells a sideways swipe from a scroll', () => {
    expect(isSideways(30, 5)).toBe(true);
    expect(isSideways(-30, 5)).toBe(true);
    expect(isSideways(30, 25)).toBe(false);
    expect(isSideways(5, 0)).toBe(false);
  });

  it('follows the finger up to a limit', () => {
    expect(slide(40)).toBe(40);
    expect(slide(500)).toBe(SWIPE_MAX_PX);
    expect(slide(-500)).toBe(-SWIPE_MAX_PX);
  });

  it('only a long enough swipe counts', () => {
    expect(swipeEnd(SWIPE_DONE_PX)).toBe('add');
    expect(swipeEnd(-SWIPE_DONE_PX)).toBe('reveal');
    expect(swipeEnd(SWIPE_DONE_PX - 1)).toBe('none');
    expect(swipeEnd(0)).toBe('none');
  });
});
