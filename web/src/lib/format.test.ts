import { describe, expect, it } from 'vitest';
import {
  addDays,
  dayMonth,
  daysBetween,
  fmtHours,
  fmtMinutes,
  isWeekend,
  longDay,
  relativeDay,
  toIso,
  weekdayShort,
} from './format';
import { easeOut, motionScale, tweenAt } from './motion';

describe('display formatting', () => {
  it('formats hours like the app does', () => {
    const cases: [number, string][] = [
      [0, '0'],
      [90, '1.5'],
      [75, '1.25'],
      [20, '0.33'],
      [60, '1'],
      [480, '8'],
      [1, '0.02'],
    ];
    for (const [m, text] of cases) expect(fmtHours(m), String(m)).toBe(text);
    expect(fmtHours(-5)).toBe('0');
  });

  it('formats minutes for buttons and toasts', () => {
    expect(fmtMinutes(15)).toBe('15m');
    expect(fmtMinutes(60)).toBe('1h');
    expect(fmtMinutes(90)).toBe('1h 30m');
    expect(fmtMinutes(120)).toBe('2h');
  });

  it('handles local dates', () => {
    expect(toIso(new Date(2026, 9, 15))).toBe('2026-10-15');
    expect(addDays('2026-10-15', -15)).toBe('2026-09-30');
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(daysBetween('2026-10-15', '2026-10-12')).toBe(-3);
    expect(longDay('2026-10-15')).toBe('Thursday 15 October');
    expect(dayMonth('2026-10-12')).toBe('12 October');
    expect(weekdayShort('2026-10-18')).toBe('Sun');
    expect(isWeekend('2026-10-17')).toBe(true);
    expect(isWeekend('2026-10-18')).toBe(true);
    expect(isWeekend('2026-10-19')).toBe(false);
    expect(relativeDay('2026-10-15', '2026-10-15')).toBe('Today');
    expect(relativeDay('2026-10-14', '2026-10-15')).toBe('Yesterday');
    expect(relativeDay('2026-10-01', '2026-10-15')).toBe('Looking back');
  });
});

describe('motion', () => {
  it('scales by the setting', () => {
    expect(motionScale('still')).toBe(0);
    expect(motionScale('calm')).toBe(1);
    expect(motionScale('lively')).toBe(1.35);
  });

  it('eases numbers from one value to another', () => {
    expect(easeOut(0)).toBe(0);
    expect(easeOut(1)).toBe(1);
    expect(easeOut(2)).toBe(1);
    expect(easeOut(0.5)).toBeCloseTo(0.875);
    expect(tweenAt(0, 480, 0)).toBe(0);
    expect(tweenAt(0, 480, 1)).toBe(480);
    expect(tweenAt(480, 0, 0.5)).toBeCloseTo(60);
  });
});
