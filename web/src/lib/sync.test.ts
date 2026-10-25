import { describe, expect, it } from 'vitest';
import type { SyncReport } from './api/DataApi';
import { nothingNew, reportLines } from './sync';

const empty: SyncReport = {
  createdAt: '2026-10-27T17:02:00Z',
  newEntries: 0,
  changedEntries: 0,
  newNotes: 0,
  changedNotes: 0,
  newProjects: 0,
  changedProjects: 0,
  other: 0,
  deleted: 0,
  keptHere: 0,
  leftOut: 0,
  clashes: [],
  clashCount: 0,
};

describe('sync report', () => {
  it('leaves out what is zero', () => {
    expect(reportLines(empty)).toEqual([]);
    expect(nothingNew(empty)).toBe(true);
  });

  it('reads like the mockup', () => {
    const r = { ...empty, newEntries: 12, changedEntries: 3, newNotes: 1, changedNotes: 2, newProjects: 1, deleted: 1 };
    expect(reportLines(r)).toEqual([
      { label: 'New entries', value: '12' },
      { label: 'Changed entries', value: '3' },
      { label: 'Notes', value: '1 new · 2 changed' },
      { label: 'Projects', value: '1 new' },
      { label: 'Deleted (can be brought back)', value: '1' },
    ]);
    expect(nothingNew(r)).toBe(false);
  });

  it('kept and left out alone are nothing new', () => {
    expect(nothingNew({ ...empty, keptHere: 4, leftOut: 1 })).toBe(true);
  });
});
