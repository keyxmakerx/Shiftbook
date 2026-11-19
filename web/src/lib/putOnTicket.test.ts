import { describe, expect, it } from 'vitest';
import type { CanvasCard, CanvasLink } from './api';
import { MAX_TICKET_CHOICES, moreLines, ticketChoices } from './putOnTicket';

const base = {
  x: 0, y: 0, z: 0, placed: false, timelineX: 0, timelineY: 0, inBox: null, stack: null,
  entry: null, note: null, inbox: null, work: [], workMinutes: null, folded: [], justToday: true,
};
const ticket = (id: string, date: string, done = false): CanvasCard => ({
  ...base,
  kind: 'tray',
  id,
  tray: {
    date,
    item: { id, date, ticket: id, minutes: 30, note: null, done, projectId: null, recurringId: null, links: [], todos: 0 },
  },
});
const jot: CanvasCard = { ...base, kind: 'note', id: 'n1', tray: null };

describe('ticketChoices', () => {
  it('offers the linked ticket first, then open ones, newest first', () => {
    const cards = [ticket('A', '2026-11-16', true), ticket('B', '2026-11-17'), ticket('C', '2026-11-18'), jot];
    const links: CanvasLink[] = [{ a: { kind: 'tray', id: 'A' }, b: { kind: 'note', id: 'n1' }, reason: null, drawn: false }];
    expect(ticketChoices(jot, cards, links).map((c) => c.id)).toEqual(['A', 'C', 'B']);
  });

  it('offers only ticket cards, and not too many', () => {
    const many = Array.from({ length: 12 }, (_, i) => ticket(`T${i}`, '2026-11-18'));
    const got = ticketChoices(jot, [...many, jot], []);
    expect(got).toHaveLength(MAX_TICKET_CHOICES);
    expect(got.every((c) => c.kind === 'tray')).toBe(true);
  });
});

describe('moreLines', () => {
  it('counts the lines under the first, leaving out blank ones', () => {
    expect(moreLines(null)).toBe(0);
    expect(moreLines('Window is Friday')).toBe(0);
    expect(moreLines('Window is Friday\n\nSummary of work\nUpgrade the VPN client\n')).toBe(2);
  });
});
