import { describe, expect, it } from 'vitest';
import {
  addDays,
  dayMonth,
  dayDistance,
  dayMonthYear,
  daysBetween,
  fmtMinutes,
  isWeekend,
  longDay,
  onDay,
  relativeDay,
  toIso,
  weekdayShort,
} from './format';
import { easeOut, motionScale, tweenAt } from './motion';

describe('display formatting', () => {
  it('writes dates in another year in full', () => {
    expect(dayMonthYear('2025-11-03')).toBe('3 November 2025');
  });

  it('formats minutes for buttons and toasts', () => {
    expect(fmtMinutes(15)).toBe('15m');
    expect(fmtMinutes(60)).toBe('1h');
    expect(fmtMinutes(90)).toBe('1h 30m');
    expect(fmtMinutes(120)).toBe('2h');
    expect(fmtMinutes(0)).toBe('0m');
    expect(fmtMinutes(45)).toBe('45m');
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
    expect(relativeDay('2026-10-16', '2026-10-15')).toBe('Tomorrow');
    expect(relativeDay('2026-10-01', '2026-10-15')).toBe('Thu 1 October');
    expect(relativeDay('2026-11-02', '2026-10-15')).toBe('Mon 2 November');
    expect(relativeDay('2025-12-31', '2026-10-15')).toBe('Wed 31 December 2025');
  });

  it('says a day mid-sentence without a stray capital', () => {
    expect(onDay('2026-10-15', '2026-10-15')).toBe('today');
    expect(onDay('2026-10-14', '2026-10-15')).toBe('yesterday');
    expect(onDay('2026-10-16', '2026-10-15')).toBe('tomorrow');
    expect(onDay('2026-10-01', '2026-10-15')).toBe('Thu 1 October');
  });

  it('says how far a day is from today', () => {
    expect(dayDistance('2026-10-15', '2026-10-15')).toBe('Today');
    expect(dayDistance('2026-10-14', '2026-10-15')).toBe('Yesterday');
    expect(dayDistance('2026-10-16', '2026-10-15')).toBe('Tomorrow');
    expect(dayDistance('2026-10-07', '2026-10-15')).toBe('8 days ago');
    expect(dayDistance('2026-10-18', '2026-10-15')).toBe('In 3 days');
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
