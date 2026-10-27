import { describe, expect, it } from 'vitest';
import { counts, findAll, formatBook, looksLikeText, paint } from './mop';

describe('MOP notebook display', () => {
  it('colours headings, steps, checks and blanks', () => {
    const lines = paint('# Prep [[site]]\n1. Back up\nVerify: ok\n## Rollback\n2) Undo\n2026 notes\nplain');
    expect(lines[0]).toEqual([
      { text: '# Prep ', tone: 'h' },
      { text: '[[site]]', tone: 'blank' },
    ]);
    expect(lines[1]![0]).toEqual({ text: '1.', tone: 'num' });
    expect(lines[2]).toEqual([{ text: 'Verify: ok', tone: 'v' }]);
    expect(lines[3]![0]!.tone).toBe('rb');
    expect(lines[4]![0]).toEqual({ text: '2)', tone: 'rb' });
    expect(lines[5]).toEqual([{ text: '2026 notes', tone: '' }]);
    expect(lines[6]).toEqual([{ text: 'plain', tone: '' }]);
    expect(paint('')).toEqual([[{ text: '', tone: '' }]]);
    expect(paint('[[ ]] and [[a]]')[0]!.map((p) => p.tone)).toEqual(['', 'blank']);
  });

  it('counts lines, characters and the selection', () => {
    const text = 'ab\ncdé\n';
    expect(counts(text, 4, 4)).toEqual({ lines: 3, chars: 7, line: 2, col: 2, selChars: 0, selLines: 0 });
    expect(counts(text, 1, 5)).toMatchObject({ selChars: 4, selLines: 2 });
  });

  it('finds every match ignoring case', () => {
    expect(findAll('Verify verify VERIFY', 'verify')).toEqual([0, 7, 14]);
    expect(findAll('abc', '')).toEqual([]);
  });

  it('writes the notebook pref in the stored form', () => {
    expect(
      formatBook({ shown: true, mode: 'open', x: 1.4, y: 2, w: 600, h: 400, view: 'checklist', numbers: false, tabs: ['a', 'b'], active: 'b' }),
    ).toBe('1 open 1 2 600 400 checklist 0 b a,b');
    expect(formatBook({ shown: false, mode: 'min', x: 0, y: 0, w: 1, h: 1, view: 'numbered', numbers: true, tabs: [], active: null })).toBe(
      '0 min 0 0 1 1 numbered 1 - -',
    );
  });

  it('guesses which files are text', () => {
    expect(looksLikeText('Core swap.txt')).toBe(true);
    expect(looksLikeText('README')).toBe(true);
    expect(looksLikeText('change.docx')).toBe(false);
  });
});
