import { describe, expect, it } from 'vitest';
import { ageOf, agesByTicket, ageWords } from './age';

const ages = agesByTicket([
  { ticket: 'INC0041201', since: '2026-11-12', days: 6, stale: true },
  { ticket: 'REQ0030455', since: '2026-11-17', days: 1, stale: false },
  { ticket: 'INC0041255', since: '2026-11-18', days: 0, stale: false },
]);

describe('ticket age', () => {
  it('finds a ticket whatever its case', () => {
    expect(ageOf(ages, 'inc0041201')?.days).toBe(6);
    expect(ageOf(ages, 'CHG1')).toBeUndefined();
  });
  it('says how long in days, and nothing on the first day', () => {
    expect(ageWords(ageOf(ages, 'INC0041201'))).toBe('open 6 days');
    expect(ageWords(ageOf(ages, 'REQ0030455'))).toBe('open 1 day');
    expect(ageWords(ageOf(ages, 'INC0041255'))).toBe('');
    expect(ageWords(undefined)).toBe('');
  });
});
