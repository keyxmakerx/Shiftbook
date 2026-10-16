import { describe, expect, it } from 'vitest';
import type { Entry } from './api';
import { repeatCandidates } from './repeat';

function entry(date: string, description: string, minutes: number, isTray = false): Entry {
  return {
    id: `${date}-${description}`,
    date,
    minutes,
    hours: String(minutes / 60),
    description,
    projectId: null,
    category: null,
    notes: null,
    timesheetCodeId: null,
    tickets: [],
    isTray,
  };
}

describe('repeatCandidates', () => {
  it('offers the latest earlier day, biggest first', () => {
    const earlier = [
      entry('2026-10-14', 'Old thing', 60),
      entry('2026-10-15', 'Daily meeting', 15),
      entry('2026-10-15', 'General tickets', 60),
    ];
    expect(repeatCandidates('2026-10-16', earlier, []).map((e) => e.description)).toEqual([
      'General tickets',
      'Daily meeting',
    ]);
  });

  it('skips the weekend gap: Monday offers Friday', () => {
    const earlier = [entry('2026-10-09', 'Daily meeting', 15)];
    expect(repeatCandidates('2026-10-12', earlier, [])).toHaveLength(1);
  });

  it('leaves out the tray, empty titles and what is already logged today', () => {
    const earlier = [
      entry('2026-10-15', 'Tickets', 90, true),
      entry('2026-10-15', '  ', 30),
      entry('2026-10-15', 'Daily meeting', 15),
      entry('2026-10-15', 'Patching', 120),
    ];
    const today = [entry('2026-10-16', 'daily meeting ', 15)];
    expect(repeatCandidates('2026-10-16', earlier, today).map((e) => e.description)).toEqual(['Patching']);
  });

  it('ignores days on or after the day being logged, and repeats of one title', () => {
    const earlier = [
      entry('2026-10-16', 'Same day', 30),
      entry('2026-10-15', 'Patching', 60),
      entry('2026-10-15', 'Patching', 30),
    ];
    expect(repeatCandidates('2026-10-16', earlier, []).map((e) => e.minutes)).toEqual([60]);
  });

  it('is empty with no earlier entries and caps at four', () => {
    expect(repeatCandidates('2026-10-16', [], [])).toEqual([]);
    const many = ['a', 'b', 'c', 'd', 'e'].map((t) => entry('2026-10-15', t, 30));
    expect(repeatCandidates('2026-10-16', many, [])).toHaveLength(4);
  });
});
