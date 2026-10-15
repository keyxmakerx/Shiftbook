import { describe, expect, it } from 'vitest';
import { checkIdle, DEFAULT_IDLE_MS, recordActivity, SLEEP_GAP_MS, startIdle, TICK_MS } from './idle';

describe('idle auto-lock', () => {
  it('defaults to 15 minutes', () => {
    expect(DEFAULT_IDLE_MS).toBe(15 * 60 * 1000);
  });

  it('locks after the idle time with steady ticks and no activity', () => {
    let state = startIdle(0);
    let lock = null;
    let now = 0;
    while (lock === null) {
      now += TICK_MS;
      ({ state, lock } = checkIdle(state, now));
    }
    expect(lock).toBe('idle');
    expect(now).toBe(DEFAULT_IDLE_MS);
  });

  it('activity pushes the lock back', () => {
    let state = startIdle(0);
    let now = 0;
    for (let i = 0; i < 200; i++) {
      now += TICK_MS;
      state = recordActivity(state, now);
      const result = checkIdle(state, now);
      expect(result.lock).toBeNull();
      state = result.state;
    }
  });

  it('locks when the computer was asleep (a long gap between ticks)', () => {
    const state = startIdle(0);
    expect(checkIdle(state, SLEEP_GAP_MS - 1).lock).toBeNull();
    expect(checkIdle(state, SLEEP_GAP_MS).lock).toBe('asleep');
  });

  it('honours a custom idle time', () => {
    const state = recordActivity(startIdle(0), 0);
    expect(checkIdle({ ...state, lastTick: 50_000 }, 55_000, 60_000).lock).toBeNull();
    expect(checkIdle({ ...state, lastTick: 55_000 }, 60_000, 60_000).lock).toBe('idle');
  });
});
