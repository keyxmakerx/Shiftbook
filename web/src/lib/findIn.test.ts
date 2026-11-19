import { describe, expect, it } from 'vitest';
import { countWords, joinPieces, pieceAt, startAt, timeStamp, wrapIndex } from './findIn';
import { findAll } from './mop';

describe('find in a notepad', () => {
  it('goes round at both ends', () => {
    expect(wrapIndex(3, 3)).toBe(0);
    expect(wrapIndex(-1, 3)).toBe(2);
    expect(wrapIndex(5, 0)).toBe(0);
  });

  it('starts at the match after the cursor', () => {
    const at = findAll('bgp flap, then bgp clean, BGP again', 'bgp');
    expect(at).toEqual([0, 15, 26]);
    expect(startAt(at, 0)).toBe(0);
    expect(startAt(at, 10)).toBe(1);
    expect(startAt(at, 30)).toBe(0);
    expect(startAt([], 4)).toBe(0);
  });

  it('says how many', () => {
    expect(countWords('', 0, 0)).toBe('');
    expect(countWords('bgp', 0, 0)).toBe('No matches');
    expect(countWords('bgp', 1, 7)).toBe('2 of 7');
  });
});

describe('the time stamp', () => {
  it('is 24-hour with leading zeros', () => {
    expect(timeStamp(new Date(2026, 10, 18, 14, 5))).toBe('14:05');
    expect(timeStamp(new Date(2026, 10, 18, 0, 0))).toBe('00:00');
    expect(timeStamp(new Date(2026, 10, 18, 9, 59))).toBe('09:59');
  });
});

describe('the text of a rendered page', () => {
  it('keeps separate lines apart and maps a match back to its piece', () => {
    const { text, pieces } = joinPieces([
      { node: 'a', text: 'Pre-checks' },
      { node: 'b', text: '' },
      { node: 'c', text: 'show version' },
    ]);
    expect(text).toBe('Pre-checks\nshow version');
    // "checks show" is on two lines: not a match.
    expect(findAll(text, 'checks show')).toEqual([]);
    const [at] = findAll(text, 'version');
    expect(pieceAt(pieces, at!)).toEqual({ node: 'c', offset: 5 });
    expect(pieceAt(pieces, at! + 'version'.length)).toEqual({ node: 'c', offset: 12 });
    expect(pieceAt(pieces, 999)).toBeNull();
  });
});
