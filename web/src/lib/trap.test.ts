import { describe, expect, it } from 'vitest';
import { wrapTarget } from './trap';

describe('wrapTarget', () => {
  const list = ['a', 'b', 'c'];
  it('wraps forward from the last and back from the first', () => {
    expect(wrapTarget(list, 'c', false)).toBe('a');
    expect(wrapTarget(list, 'a', true)).toBe('c');
  });
  it('lets the browser move in the middle', () => {
    expect(wrapTarget(list, 'b', false)).toBeNull();
    expect(wrapTarget(list, 'b', true)).toBeNull();
  });
  it('pulls focus in from outside', () => {
    expect(wrapTarget(list, 'z', false)).toBe('a');
    expect(wrapTarget(list, null, true)).toBe('c');
    expect(wrapTarget([], 'a', false)).toBeNull();
  });
});
