import { describe, expect, it } from 'vitest';
import { indentEdit, type IndentEdit } from './indent';

/** Apply an edit the way the text box would. */
function apply(text: string, e: IndentEdit | null): string {
  return e ? text.slice(0, e.from) + e.insert + text.slice(e.to) : text;
}

describe('Tab in a notepad', () => {
  it('puts a tab at the cursor', () => {
    const e = indentEdit('Swap the switch', 5, 5, false);
    expect(apply('Swap the switch', e)).toBe('Swap \tthe switch');
    expect(e).toMatchObject({ start: 6, end: 6 });
  });

  it('replaces a selection inside one line with a tab', () => {
    expect(apply('ab cd', indentEdit('ab cd', 2, 3, false))).toBe('ab\tcd');
  });

  it('indents every selected line, keeping blank lines blank', () => {
    const text = 'Prep\nBack up\n\nVerify\nDone';
    const e = indentEdit(text, 6, 19, false); // from inside "Back up" to inside "Verify"
    expect(apply(text, e)).toBe('Prep\n\tBack up\n\n\tVerify\nDone');
    expect(e).toMatchObject({ start: 7, end: 21 });
  });

  it("doesn't take the next line when the selection ends at its start", () => {
    const text = 'one\ntwo\nthree';
    expect(apply(text, indentEdit(text, 0, 8, false))).toBe('\tone\n\ttwo\nthree');
  });

  it('Shift+Tab takes one tab or up to four spaces off each line', () => {
    const text = '\t\tStep 1\n      Step 2\nStep 3';
    const e = indentEdit(text, 0, text.length, true);
    expect(apply(text, e)).toBe('\tStep 1\n  Step 2\nStep 3');
  });

  it('Shift+Tab with just a cursor outdents its line', () => {
    const text = 'a\n\tb';
    const e = indentEdit(text, 4, 4, true);
    expect(apply(text, e)).toBe('a\nb');
    expect(e).toMatchObject({ start: 3, end: 3 });
  });

  it('Shift+Tab on lines with no indent does nothing', () => {
    expect(indentEdit('plain', 2, 2, true)).toBeNull();
  });
});
