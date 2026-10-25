import { describe, expect, it } from 'vitest';
import { workTickets } from './workTickets';

const entry = (id: string, date: string, tickets: { id: string; number: string; done: boolean; note: string | null }[]) =>
  ({ id, date, tickets: tickets.map((t) => ({ ...t, links: [] })) }) as never;

describe('workTickets', () => {
  it('shows each number once; its newest place says if it is done and holds its notepad', () => {
    // Storage keeps every place a number appears in step (one item per ticket, round 15).
    const card = {
      entry: entry('today', '2026-10-22', [{ id: 'a', number: 'CHG0146615', done: false, note: 'waiting on vendor' }]),
      work: [entry('old', '2026-10-20', [{ id: 'b', number: 'chg0146615', done: true, note: 'waiting on vendor' }])],
      folded: [
        { date: '2026-10-21', item: { id: 't', ticket: 'CHG0146615', done: false, note: 'waiting on vendor' } as never },
        { date: '2026-10-21', item: { id: 'u', ticket: 'INC1', done: true, note: null } as never },
      ],
    };
    const list = workTickets(card);
    expect(list.map((w) => w.number)).toEqual(['CHG0146615', 'INC1']);
    expect(list[0]!.done).toBe(false);
    expect(list[0]!.sources.map((s) => s.id)).toEqual(['a', 't', 'b']);
    expect(list[0]!.note).toBe('waiting on vendor');
    expect(list[1]!.done).toBe(true);
  });
});
