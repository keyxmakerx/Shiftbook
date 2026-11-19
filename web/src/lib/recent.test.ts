import { describe as group, expect, it } from 'vitest';
import { MAX_RECENT, describe, forget, step, visit, type Recent } from './recent';

const view = (id: string): Recent => ({ key: `view:${id}`, kind: 'view', id, title: id, word: 'view' });
const note = (id: string, title = '…'): Recent => ({ key: `note:${id}`, kind: 'note', id, title, word: 'note' });

group('jump back', () => {
  it('puts what was just opened first, once', () => {
    let list = visit([], view('desk'));
    list = visit(list, note('a'));
    list = visit(list, view('notes'));
    list = visit(list, note('a'));
    expect(list.map((r) => r.key)).toEqual(['note:a', 'view:notes', 'view:desk']);
  });

  it('forgets the oldest past the limit', () => {
    let list: Recent[] = [];
    for (let i = 0; i < MAX_RECENT + 3; i++) list = visit(list, note(String(i)));
    expect(list).toHaveLength(MAX_RECENT);
    expect(list[0]!.id).toBe(String(MAX_RECENT + 2));
    expect(list.at(-1)!.id).toBe('3');
  });

  it('names a line once its title is known', () => {
    const list = describe([note('a'), view('desk')], 'note:a', { title: 'Weekly ops · Mon 19 Oct', word: 'meeting note' });
    expect(list[0]).toMatchObject({ key: 'note:a', title: 'Weekly ops · Mon 19 Oct', word: 'meeting note' });
    expect(list[1]).toEqual(view('desk'));
  });

  it('turns a note into a page without doubling it', () => {
    const page: Recent = { key: 'page:a', kind: 'page', id: 'a', title: 'Guest Wi-Fi', word: 'page' };
    const list = describe([note('a'), view('wiki'), page], 'note:a', { kind: 'page', title: 'Guest Wi-Fi', word: 'page' });
    expect(list.map((r) => r.key)).toEqual(['page:a', 'view:wiki']);
  });

  it('forgets a thing that is gone', () => {
    expect(forget([note('a'), view('desk')], 'note:a')).toEqual([view('desk')]);
  });

  it('steps round the ends', () => {
    expect(step(0, 1, 3)).toBe(1);
    expect(step(2, 1, 3)).toBe(0);
    expect(step(0, -1, 3)).toBe(2);
    expect(step(0, 1, 0)).toBe(0);
  });
});
