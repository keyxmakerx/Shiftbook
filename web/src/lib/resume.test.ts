import { describe, expect, it } from 'vitest';
import type { Entry, ResumeCard } from './api/DataApi';
import { cardLead, cardTitle, carryOnText, lastLogged, noticeReturn } from './resume';

function entry(description: string, tickets: string[], notes: string | null = null): Entry {
  return {
    id: 'e1',
    date: '2026-10-16',
    minutes: 90,
    hours: '1.5',
    description,
    projectId: null,
    category: null,
    notes,
    timesheetCodeId: null,
    tickets: tickets.map((number, i) => ({ id: `t${i}`, number, links: [] })),
    isTray: false,
  };
}

const list = (texts: string[]): ResumeCard => ({
  trigger: 'unlocked',
  list: texts.map((text, i) => ({ id: `n${i}`, text, done: false, from: null })),
  entry: null,
  changedAt: null,
});

const fromEntry = (e: Entry, trigger: ResumeCard['trigger'] = 'break'): ResumeCard => ({
  trigger,
  list: [],
  entry: e,
  changedAt: '2026-10-16T11:00:00Z',
});

describe('Where was I?', () => {
  it('only reports returns after a minute or more', () => {
    expect(noticeReturn(0, 59_000)).toEqual({ lastActivity: 59_000, awayMinutes: null });
    expect(noticeReturn(0, 52 * 60_000 + 30_000)).toEqual({ lastActivity: 3_150_000, awayMinutes: 52 });
  });

  it('carries on with the tickets and title, without repeating a ticket', () => {
    expect(carryOnText(fromEntry(entry('Review rule set for VLAN 40', ['CHG0012345'])))).toBe(
      'CHG0012345 Review rule set for VLAN 40',
    );
    expect(carryOnText(fromEntry(entry('Fix CHG0012345 rollback', ['CHG0012345'])))).toBe(
      'Fix CHG0012345 rollback',
    );
    expect(carryOnText(list(['Chase the printer tickets', 'Renew certs']))).toBe('Chase the printer tickets');
  });

  it('words the card for the list, a break or another return', () => {
    expect(cardLead(list(['a']))).toBe("You said you'd start with");
    expect(cardLead(fromEntry(entry('x', [])))).toBe('Before your break');
    expect(cardLead(fromEntry(entry('x', []), 'unlocked'))).toBe('Last time');
    expect(cardTitle(fromEntry(entry('', ['INC1', 'INC2'])))).toBe('INC1, INC2');
  });

  it('says how long ago something was logged', () => {
    const now = new Date('2026-10-16T11:52:00Z');
    expect(lastLogged('2026-10-16T11:00:00Z', now)).toBe('Last logged 52 minutes ago');
    expect(lastLogged('2026-10-16T11:51:30Z', now)).toBe('Last logged just now');
    expect(lastLogged('2026-10-16T08:00:00Z', now)).toBe('Last logged 3 hours ago');
    expect(lastLogged('2026-10-15T09:00:00Z', now)).toBe('Last logged yesterday');
    expect(lastLogged('2026-10-12T09:00:00Z', now)).toBe('Last logged 4 days ago');
    expect(lastLogged(null, now)).toBe('');
  });
});
