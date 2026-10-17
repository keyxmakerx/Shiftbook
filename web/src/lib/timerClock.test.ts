import { describe, expect, it } from 'vitest';
import { clockText, noticeActivity, seconds, startClock, tickClock } from './timerClock';

describe('timer clock', () => {
  it('counts ticks and skips a sleep', () => {
    let c = startClock(0);
    c = tickClock(c, 1000);
    c = tickClock(c, 2000);
    expect(seconds(c)).toBe(2);
    c = tickClock(c, 2000 + 10 * 60_000); // the computer slept
    expect(seconds(c)).toBe(2);
    c = tickClock(c, 3000 + 10 * 60_000);
    expect(seconds(c)).toBe(3);
  });

  it('keeps going from a paused count', () => {
    const c = tickClock(startClock(500, 3_700_000, [400]), 1500);
    expect(seconds(c)).toBe(3701);
    expect(c.quiet).toEqual([400]);
  });

  it('notes quiet stretches of a minute or more', () => {
    let c = startClock(0);
    c = noticeActivity(c, 30_000);
    expect(c.quiet).toEqual([]);
    c = noticeActivity(c, 30_000 + 31 * 60_000);
    expect(c.quiet).toEqual([1860]);
  });

  it('shows the time like a clock', () => {
    expect(clockText(7)).toBe('0:07');
    expect(clockText(725)).toBe('12:05');
    expect(clockText(3729)).toBe('1:02:09');
  });
});
