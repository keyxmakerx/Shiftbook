import { describe, expect, it } from 'vitest';
import { archiveWarning, changedOn, highlight, snippet } from './notes';

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
});

describe('archiveWarning', () => {
  it('says when', () => {
    expect(archiveWarning(1)).toBe('This note moves to the archive tomorrow');
    expect(archiveWarning(5)).toBe('This note moves to the archive in 5 days');
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
