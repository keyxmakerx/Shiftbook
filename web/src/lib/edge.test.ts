import { describe, expect, it } from 'vitest';
import { edge } from './edge';

describe('edge', () => {
  it('says on once when the value turns true, however often it is asked again', () => {
    const flipped = edge();
    expect(flipped(false)).toBeNull();
    expect(flipped(true)).toBe('on');
    // The effect re-running while the box stays open (a keystroke) must not reopen it.
    expect(flipped(true)).toBeNull();
    expect(flipped(true)).toBeNull();
  });

  it('says off once when it turns false, then on again the next time it opens', () => {
    const flipped = edge();
    flipped(true);
    expect(flipped(false)).toBe('off');
    expect(flipped(false)).toBeNull();
    expect(flipped(true)).toBe('on');
  });

  it('can start out true', () => {
    const flipped = edge(true);
    expect(flipped(true)).toBeNull();
    expect(flipped(false)).toBe('off');
  });
});
