import { describe, expect, it } from 'vitest';
import type { OffDay, TimeOffPlan } from './api/DataApi';
import { offButton, offDone, offSkipped, offSummary } from './timeOff';

function plan(days: OffDay[]): TimeOffPlan {
  const booked = days.filter((d) => d.skip === null);
  const minutes = booked.reduce((sum, d) => sum + d.minutes, 0);
  return { days, booked: booked.length, minutes, hours: String(minutes / 60) };
}

const on = (date: string): OffDay => ({ date, minutes: 480, skip: null });
const free = (date: string): OffDay => ({ date, minutes: 0, skip: 'free' });
const logged = (date: string): OffDay => ({ date, minutes: 0, skip: 'logged' });

describe('time off words', () => {
  it('sums up a week', () => {
    const week = plan([on('2026-11-02'), on('2026-11-03'), logged('2026-11-04'), on('2026-11-05'), on('2026-11-06'), free('2026-11-07'), free('2026-11-08')]);
    expect(offSummary(week)).toBe('4 work days, 32 h');
    expect(offSkipped(week)).toBe("Skipped: 2 days you don't usually work and 1 day you already logged.");
    expect(offButton(week)).toBe('Book 4 days');
  });

  it('says one day as one day', () => {
    const one = plan([on('2026-11-02')]);
    expect(offSummary(one)).toBe('1 work day, 8 h');
    expect(offSkipped(one)).toBe('');
    expect(offButton(one)).toBe('Book 1 day');
  });

  it('explains when there is nothing to book', () => {
    const weekend = plan([free('2026-11-07'), free('2026-11-08')]);
    expect(offSummary(weekend)).toBe('');
    expect(offSkipped(weekend)).toMatch(/^Nothing to book: these are days you don't usually work/);
    expect(offSkipped(plan([logged('2026-11-04')]))).toMatch(/has something logged already/);
    expect(offButton(weekend)).toBe('Book');
    expect(offButton(null)).toBe('Book');
  });

  it('words the toast', () => {
    expect(offDone(['2026-11-02'], '8')).toBe('Mon 2 booked off');
    expect(offDone(['2026-11-02', '2026-11-03'], '16')).toBe('2 days booked off (16 h)');
  });
});
