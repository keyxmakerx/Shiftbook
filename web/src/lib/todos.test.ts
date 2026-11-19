import { describe, expect, it } from 'vitest';
import type { IndentEdit } from './indent';
import { clickBox, closeBracket, DONE_BOX, enterOnTodo, OPEN_BOX } from './todos';

/** The text after an edit, with `|` where the cursor ends up. */
function after(text: string, edit: IndentEdit | null): string | null {
  if (!edit) return null;
  const next = text.slice(0, edit.from) + edit.insert + text.slice(edit.to);
  return `${next.slice(0, edit.start)}|${next.slice(edit.start)}`;
}

describe('closeBracket', () => {
  it('turns [] at the start of a line into a box', () => {
    expect(after('[', closeBracket('[', 1))).toBe(`${OPEN_BOX} |`);
    expect(after('first\n  [ ', closeBracket('first\n  [ ', 10))).toBe(`first\n  ${OPEN_BOX} |`);
  });
  it('turns [x] into a ticked box', () => {
    expect(after('[x', closeBracket('[x', 2))).toBe(`${DONE_BOX} |`);
  });
  it('swaps a box Enter just made for the one typed', () => {
    const pad = `${OPEN_BOX} one\n${OPEN_BOX} [x`;
    expect(after(pad, closeBracket(pad, pad.length))).toBe(`${OPEN_BOX} one\n${DONE_BOX} |`);
  });
  it('keeps the space words already have', () => {
    expect(after('[ call the vendor', closeBracket('[ call the vendor', 1))).toBe(`${OPEN_BOX} |call the vendor`);
  });
  it('leaves brackets in the middle of a line alone', () => {
    expect(closeBracket('see ports [', 11)).toBeNull();
    expect(closeBracket('[12', 3)).toBeNull();
  });
});

describe('clickBox', () => {
  const pad = `notes\n${OPEN_BOX} Update the port map\n\t${DONE_BOX} Labels`;
  it('ticks an open box and unticks a ticked one', () => {
    expect(after(pad, clickBox(pad, 6))).toBe(`notes\n|${DONE_BOX} Update the port map\n\t${DONE_BOX} Labels`);
    expect(after(pad, clickBox(pad, 7))).toBe(`notes\n${DONE_BOX}| Update the port map\n\t${DONE_BOX} Labels`);
    const last = pad.lastIndexOf(DONE_BOX);
    expect(after(pad, clickBox(pad, last + 1))).toBe(`notes\n${OPEN_BOX} Update the port map\n\t${OPEN_BOX}| Labels`);
  });
  it('does nothing in the words or on other lines', () => {
    expect(clickBox(pad, 2)).toBeNull();
    expect(clickBox(pad, 10)).toBeNull();
    expect(clickBox(`a ${OPEN_BOX} b`, 2)).toBeNull();
  });
});

describe('enterOnTodo', () => {
  it('starts the next to-do, with the same indent', () => {
    const pad = `\t${OPEN_BOX} Update the port map`;
    expect(after(pad, enterOnTodo(pad, pad.length))).toBe(`${pad}\n\t${OPEN_BOX} |`);
    const ticked = `${DONE_BOX} Labels\nmore`;
    expect(after(ticked, enterOnTodo(ticked, 8))).toBe(`${DONE_BOX} Labels\n${OPEN_BOX} |\nmore`);
  });
  it('takes an empty box away', () => {
    const pad = `${OPEN_BOX} one\n${OPEN_BOX} `;
    expect(after(pad, enterOnTodo(pad, pad.length))).toBe(`${OPEN_BOX} one\n|`);
  });
  it('is a plain Enter in the middle of the words and on other lines', () => {
    const pad = `${OPEN_BOX} Update the port map`;
    expect(enterOnTodo(pad, 8)).toBeNull();
    expect(enterOnTodo('plain line', 10)).toBeNull();
  });
});
