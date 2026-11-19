import { describe, expect, it } from 'vitest';
import { archiveWarning, changedOn, highlight, matchingLine, snippet } from './notes';

describe('highlight', () => {
  it('marks every typed word, ignoring case', () => {
    expect(highlight('UPS battery swap at the UPS', 'ups swap')).toEqual([
      { text: 'UPS', hit: true },
      { text: ' battery ', hit: false },
      { text: 'swap', hit: true },
      { text: ' at the ', hit: false },
      { text: 'UPS', hit: true },
    ]);
  });

  it('leaves text alone without a query', () => {
    expect(highlight('Docks', '  ')).toEqual([{ text: 'Docks', hit: false }]);
    expect(highlight('', 'x')).toEqual([]);
  });
});

describe('snippet', () => {
  it('skips a first line that repeats the title', () => {
    expect(snippet('Guest WiFi\nThe password is on the fridge', 'Guest WiFi', '')).toBe(
      'The password is on the fridge',
    );
  });

  it('centres on a match far into the body', () => {
    const body = 'word '.repeat(60) + 'vlan forty was guest';
    const s = snippet(body, 'Plan', 'vlan', 60);
    expect(s.startsWith('…')).toBe(true);
    expect(s).toContain('vlan forty');
  });

  it('shortens long text', () => {
    expect(snippet('a'.repeat(200), 'x', '', 10)).toBe('aaaaaaaaaa…');
  });

  it('carries on after a title that was cut short, instead of repeating it', () => {
    const line = 'Do we need to look at the upgrades again since it is the start of the month?';
    expect(snippet(`${line}\nAsk Sam`, 'Do we need to look at the upgrades again…', '')).toBe(
      '… since it is the start of the month? Ask Sam',
    );
    // Nothing after the cut, or only its full stop: no taste at all rather than "…?".
    expect(snippet('Do we need to look', 'Do we need to look…', '')).toBe('');
    expect(snippet('Do we need to look?', 'Do we need to look…', '')).toBe('');
  });

  it('skips a first line that repeats the title with other spacing', () => {
    expect(snippet('(GOAL)  work on the\tswitch\nfor the heartbeat', '(GOAL) work on the switch', '')).toBe(
      'for the heartbeat',
    );
  });
});

describe('archiveWarning', () => {
  it('says when', () => {
    expect(archiveWarning(1)).toBe('This note moves to the archive tomorrow');
    expect(archiveWarning(5)).toBe('This note moves to the archive in 5 days');
  });
});

describe('matchingLine', () => {
  const body = 'Pre-checks\nshow ip bgp summary on edge-rtr-02\nRollback: reload in 10';

  it('gives the line a searched word is on', () => {
    expect(matchingLine(body, 'bgp')).toBe('show ip bgp summary on edge-rtr-02');
    expect(matchingLine(body, 'ROLLBACK')).toBe('Rollback: reload in 10');
  });

  it('is empty when no line has the word, or nothing was asked', () => {
    expect(matchingLine(body, 'ospf')).toBe('');
    expect(matchingLine(body, '  ')).toBe('');
  });

  it('cuts a long line around the word', () => {
    const long = 'x '.repeat(60) + 'needle ' + 'y '.repeat(60);
    const line = matchingLine(long, 'needle', 40);
    expect(line).toContain('needle');
    expect(line.startsWith('…')).toBe(true);
    expect(line.endsWith('…')).toBe(true);
    expect(line.length).toBeLessThanOrEqual(42);
  });
});

describe('changedOn', () => {
  it('names near days and dates further back', () => {
    expect(changedOn('2026-10-17T12:00:00Z', '2026-10-17')).toBe('Today');
    expect(changedOn('2026-10-16T12:00:00Z', '2026-10-17')).toBe('Yesterday');
    expect(changedOn('2026-10-02T12:00:00Z', '2026-10-17')).toBe('Fri 2 October');
    expect(changedOn('2025-12-24T12:00:00Z', '2026-10-17')).toBe('Wed 24 December 2025');
  });
});
