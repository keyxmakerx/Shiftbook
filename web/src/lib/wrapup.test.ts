import { describe, expect, it } from 'vitest';
import { insertLine, isWrapupTime, MAX_LINES, removeLine } from './wrapup';

describe('isWrapupTime', () => {
  const at = (h: number, m: number) => new Date(2026, 9, 16, h, m);
  it('is off without a time', () => {
    expect(isWrapupTime(at(23, 0), null)).toBe(false);
    expect(isWrapupTime(at(23, 0), '')).toBe(false);
  });
  it('starts at the chosen minute', () => {
    expect(isWrapupTime(at(16, 59), '17:00')).toBe(false);
    expect(isWrapupTime(at(17, 0), '17:00')).toBe(true);
    expect(isWrapupTime(at(23, 59), '17:00')).toBe(true);
    expect(isWrapupTime(at(9, 4), '09:05')).toBe(false);
  });
});

describe('lines', () => {
  it('inserts after a line, up to the limit', () => {
    expect(insertLine(['a', 'b'], 0)).toEqual(['a', '', 'b']);
    expect(insertLine(['a'], 0)).toEqual(['a', '']);
    const full = Array.from({ length: MAX_LINES }, (_, i) => String(i));
    expect(insertLine(full, 2)).toBe(full);
  });
  it('removes a line but keeps one to type in', () => {
    expect(removeLine(['a', '', 'b'], 1)).toEqual(['a', 'b']);
    expect(removeLine([''], 0)).toEqual(['']);
  });
});
