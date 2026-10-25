import { describe, expect, it } from 'vitest';
import { agoText, MAX_CHANGES, remember, undoFrom, type Change } from './undoHistory';

function change(id: number): Change {
  return { id, text: `change ${id}`, at: id, undo: () => {}, redo: null };
}

describe('undo history', () => {
  it('keeps the newest twenty', () => {
    let list: Change[] = [];
    for (let i = 1; i <= MAX_CHANGES + 3; i++) list = remember(list, change(i));
    expect(list).toHaveLength(MAX_CHANGES);
    expect(list[0]!.id).toBe(4);
    expect(list.at(-1)!.id).toBe(MAX_CHANGES + 3);
  });

  it('undoing an older change undoes the ones after it, newest first', () => {
    const list = [1, 2, 3, 4].map(change);
    const { kept, undo } = undoFrom(list, 2);
    expect(kept.map((c) => c.id)).toEqual([1]);
    expect(undo.map((c) => c.id)).toEqual([4, 3, 2]);
    expect(undoFrom(list, 9).undo).toEqual([]);
  });

  it('says how long ago', () => {
    expect(agoText(0, 30_000)).toBe('just now');
    expect(agoText(0, 2 * 60_000)).toBe('2 min ago');
    expect(agoText(0, 65 * 60_000)).toBe('1 h ago');
  });
});
