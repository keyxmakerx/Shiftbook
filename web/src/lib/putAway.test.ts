import { describe, expect, it } from 'vitest';
import { canPutAway, refsOf, tidyCandidates } from './putAway';

type Card = Parameters<typeof tidyCandidates>[0][number] & { work: { id: string }[] };

const entry = (id: string, date: string): Card =>
  ({ kind: 'entry', id, entry: { date } as never, tray: null, work: [] }) as Card;
const ticket = (id: string, date: string, done: boolean): Card =>
  ({ kind: 'tray', id, entry: null, tray: { date, item: { done } as never }, work: [] }) as Card;
const note = (id: string): Card => ({ kind: 'note', id, entry: null, tray: null, work: [] }) as Card;

describe('putting cards away', () => {
  it('takes entries and done tickets, not notes or open tickets', () => {
    expect(canPutAway(entry('a', '2026-10-26'))).toBe(true);
    expect(canPutAway(ticket('t', '2026-10-26', true))).toBe(true);
    expect(canPutAway(ticket('t', '2026-10-26', false))).toBe(false);
    expect(canPutAway(note('n'))).toBe(false);
  });

  it('takes the other days of the same work too', () => {
    const c = { kind: 'entry' as const, id: 'a', work: [{ id: 'b' }, { id: 'c' }] };
    expect(refsOf(c as never).map((r) => r.id)).toEqual(['a', 'b', 'c']);
  });

  it('tidies up cards from before a day, leaving pinned ones', () => {
    const cards = [
      entry('today', '2026-10-27'),
      entry('old', '2026-10-20'),
      entry('pinned', '2026-10-20'),
      ticket('done', '2026-10-21', true),
      ticket('open', '2026-10-21', false),
      note('n'),
    ];
    const got = tidyCandidates(cards, '2026-10-27', new Set(['entry:pinned']));
    expect(got.map((c) => c.id)).toEqual(['old', 'done']);
  });
});
