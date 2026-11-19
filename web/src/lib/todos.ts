// To-dos inside notepads (round 21): a line that starts with a tick box is a to-do. Typing
// `[]` at the start of a line makes the box, a click on the box ticks it, and Enter at the
// end of a to-do starts the next one. What counts as a to-do (and the counts on cards and in
// the bell) is worked out in the app's core (`shiftbook_core::todos`); this file is only the
// typing.

import type { IndentEdit } from './indent';
import { applyEdit } from './indent';

export const OPEN_BOX = '☐';
export const DONE_BOX = '☑';

/** Where the line holding `at` starts and ends in `text`. */
function lineAt(text: string, at: number): { from: number; to: number } {
  const from = text.lastIndexOf('\n', at - 1) + 1;
  const nl = text.indexOf('\n', at);
  return { from, to: nl === -1 ? text.length : nl };
}

/** Where a line's box is, when the line starts with one (after any indent); otherwise -1. */
function boxOn(text: string, from: number, to: number): number {
  let i = from;
  while (i < to && (text[i] === ' ' || text[i] === '\t')) i += 1;
  return i < to && (text[i] === OPEN_BOX || text[i] === DONE_BOX) ? i : -1;
}

/**
 * A `]` typed at `caret`: when it closes `[`, `[ ` or `[x` at the start of a line, the
 * brackets become a box and a space. Returns null when it is just a `]`.
 */
export function closeBracket(text: string, caret: number): IndentEdit | null {
  const { from } = lineAt(text, caret);
  const before = text.slice(from, caret);
  // A box Enter just put there counts as the start of the line too, so `[x]` after it works.
  const m = /^([ \t]*)(?:[☐☑] ?)?\[( |x|X)?$/.exec(before);
  if (!m) return null;
  const box = m[2] === 'x' || m[2] === 'X' ? DONE_BOX : OPEN_BOX;
  const at = from + m[1].length;
  // Words already there keep their own space.
  const insert = text[caret] === ' ' ? box : `${box} `;
  const end = at + 2;
  return { from: at, to: caret, insert, start: end, end };
}

/**
 * A click that left the cursor at `caret`: on a line's box (just before or just after it)
 * it ticks the box, or unticks it. Returns null anywhere else.
 */
export function clickBox(text: string, caret: number): IndentEdit | null {
  const { from, to } = lineAt(text, caret);
  const at = boxOn(text, from, to);
  if (at === -1 || (caret !== at && caret !== at + 1)) return null;
  const insert = text[at] === OPEN_BOX ? DONE_BOX : OPEN_BOX;
  return { from: at, to: at + 1, insert, start: caret, end: caret };
}

/**
 * Enter at `caret`: at the end of a to-do it starts the next one on a new line; on a box
 * with no words yet it takes the box away (so Enter twice ends the list). Returns null when
 * Enter should just be Enter.
 */
export function enterOnTodo(text: string, caret: number): IndentEdit | null {
  const { from, to } = lineAt(text, caret);
  const at = boxOn(text, from, to);
  if (at === -1 || caret < at + 1) return null;
  const words = text.slice(at + 1, to).trim();
  if (words === '') {
    return { from: at, to, insert: '', start: at, end: at };
  }
  if (text.slice(caret, to).trim() !== '') return null;
  const insert = `\n${text.slice(from, at)}${OPEN_BOX} `;
  const end = caret + insert.length;
  return { from: caret, to, insert, start: end, end };
}

/**
 * Svelte action for a notepad: `[]` makes a tick box, a click ticks it, Enter carries the
 * list on. Pass `{ lines: false }` where Enter already means something else.
 */
export function todoBoxes(box: HTMLTextAreaElement, opts: { lines?: boolean } = {}) {
  let lines = opts.lines !== false;
  function plain(e: KeyboardEvent): boolean {
    return !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing && box.selectionStart === box.selectionEnd;
  }
  // Where the cursor is right after `[]` became a box: the space most people type next is
  // already there, so that one is skipped.
  let made = -1;
  function onkeydown(e: KeyboardEvent) {
    const justMade = made;
    made = -1;
    if (!plain(e)) return;
    if (e.key === ' ' && justMade === box.selectionStart) {
      e.preventDefault();
      return;
    }
    let edit: IndentEdit | null = null;
    if (e.key === ']') edit = closeBracket(box.value, box.selectionStart);
    else if (e.key === 'Enter' && !e.shiftKey && lines) edit = enterOnTodo(box.value, box.selectionStart);
    if (!edit) return;
    e.preventDefault();
    applyEdit(box, edit);
    if (e.key === ']') made = box.selectionStart;
  }
  function onclick(e: MouseEvent) {
    made = -1;
    // A plain click only: a drag, or Shift+click to select, leaves the box alone.
    if (e.shiftKey || e.detail > 1 || box.selectionStart !== box.selectionEnd) return;
    const edit = clickBox(box.value, box.selectionStart);
    if (edit) applyEdit(box, edit);
  }
  box.addEventListener('keydown', onkeydown);
  box.addEventListener('click', onclick);
  return {
    update(next: { lines?: boolean } = {}) {
      lines = next.lines !== false;
    },
    destroy() {
      box.removeEventListener('keydown', onkeydown);
      box.removeEventListener('click', onclick);
    },
  };
}
