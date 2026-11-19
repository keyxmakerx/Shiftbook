import { describe, expect, it } from 'vitest';
import type { Capture, CaptureLine } from './api/DataApi';
import { fillTimes, isManyLines, manyRows, missingTimes, oneLine, totalMinutes } from './manyLines';

function line(text: string, minutes: number | null, tickets: string[] = []): CaptureLine {
  const capture: Capture = {
    minutes,
    hoursText: minutes === null ? null : `${minutes}m`,
    tickets,
    projectId: null,
    recurringId: null,
    projectQuery: null,
    date: '2026-11-18',
    dayWord: null,
    description: text,
  };
  return { text, capture };
}

describe('several lines in Log work', () => {
  const rows = manyRows([line('patched print server', 45), line('chased the vendor', null, ['INC0041201']), line('order the part', null)]);

  it('starts each row with the time its line had', () => {
    expect(rows.map((r) => r.minutes)).toEqual([45, null, null]);
    expect(rows[1]?.tickets).toEqual(['INC0041201']);
    expect(missingTimes(rows)).toBe(2);
    expect(totalMinutes(rows)).toBe(45);
  });

  it('fills only the rows with no time yet', () => {
    const filled = fillTimes(rows, 15);
    expect(filled.map((r) => r.minutes)).toEqual([45, 15, 15]);
    expect(missingTimes(filled)).toBe(0);
    expect(totalMinutes(filled)).toBe(75);
  });

  it('knows a paste of several lines from one line', () => {
    expect(isManyLines('one\n\ntwo\n')).toBe(true);
    expect(isManyLines('just one\n')).toBe(false);
    expect(oneLine('INC1\n  INC2\n')).toBe('INC1 INC2');
  });
});
