import { describe, expect, it } from 'vitest';
import { formatLongDate, toIsoDate } from './dates';

describe('toIsoDate', () => {
  it('uses the local calendar date with zero padding', () => {
    expect(toIsoDate(new Date(2026, 0, 5))).toBe('2026-01-05');
    expect(toIsoDate(new Date(2026, 11, 31))).toBe('2026-12-31');
  });

  it('keeps late-evening times on the same local day', () => {
    expect(toIsoDate(new Date(2026, 9, 15, 23, 59, 59))).toBe('2026-10-15');
  });
});

describe('formatLongDate', () => {
  it('includes weekday, day, month and year', () => {
    const text = formatLongDate(new Date(2026, 9, 15), 'en-GB');
    expect(text).toContain('Thursday');
    expect(text).toContain('15');
    expect(text).toContain('October');
    expect(text).toContain('2026');
  });
});
