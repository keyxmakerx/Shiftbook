// Tab in a notepad (MOPs, notes, wiki pages): Tab indents and Shift+Tab takes an indent off,
// so sections can be tabbed in the way they are in other note apps. Escape and then Tab still
// moves on to the next control, so the keyboard is never stuck in the box.

/** What an edit does to a text box: the whole new text and where the selection ends up. */
export interface IndentEdit {
  /** Where the changed stretch starts and ends in the old text. */
  from: number;
  to: number;
  /** What goes in its place. */
  insert: string;
  /** The selection afterwards, in the new text. */
  start: number;
  end: number;
}

const TAB = '\t';

/** How much indent one Shift+Tab takes off the start of `line`: a tab, or up to four spaces. */
function outdentWidth(line: string): number {
  if (line.startsWith(TAB)) return 1;
  let n = 0;
  while (n < 4 && line[n] === ' ') n += 1;
  return n;
}

/**
 * Work out a Tab (or Shift+Tab when `back`) in `text` with the selection from `start` to `end`.
 * With nothing selected on one line, Tab puts a tab at the cursor. With lines selected, Tab
 * indents each of them and Shift+Tab takes one indent off each. Returns null when there is
 * nothing to do (Shift+Tab on lines with no indent).
 */
export function indentEdit(text: string, start: number, end: number, back: boolean): IndentEdit | null {
  const a = Math.min(start, end);
  const b = Math.max(start, end);
  const lineStart = text.lastIndexOf('\n', a - 1) + 1;
  const sel = text.slice(a, b);
  if (!back && !sel.includes('\n')) {
    return { from: a, to: b, insert: TAB, start: a + 1, end: a + 1 };
  }
  // A selection that ends right at the start of a line doesn't take that line with it.
  const lastEnd = b > a && text[b - 1] === '\n' ? b - 1 : b;
  const nl = text.indexOf('\n', lastEnd);
  const blockEnd = nl === -1 ? text.length : nl;
  const lines = text.slice(lineStart, blockEnd).split('\n');
  let firstShift = 0;
  let total = 0;
  const out = lines.map((line, i) => {
    let shift: number;
    let next: string;
    if (back) {
      const w = outdentWidth(line);
      shift = -w;
      next = line.slice(w);
    } else if (line === '' && lines.length > 1) {
      shift = 0;
      next = line;
    } else {
      shift = 1;
      next = TAB + line;
    }
    if (i === 0) firstShift = shift;
    total += shift;
    return next;
  });
  if (total === 0 && back) return null;
  const insert = out.join('\n');
  // Keep the cursor on the same character; never push it before the start of its line.
  const newStart = Math.max(lineStart, a + firstShift);
  const newEnd = a === b ? newStart : Math.max(newStart, b + total);
  return { from: lineStart, to: blockEnd, insert, start: newStart, end: newEnd };
}

/** Svelte action for a textarea: Tab and Shift+Tab indent, Escape then Tab moves on. */
export function tabIndent(box: HTMLTextAreaElement) {
  let leaving = false;
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      leaving = true;
      return;
    }
    if (e.key !== 'Tab' || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) {
      leaving = false;
      return;
    }
    if (leaving) {
      leaving = false;
      return;
    }
    const edit = indentEdit(box.value, box.selectionStart, box.selectionEnd, e.shiftKey);
    e.preventDefault();
    e.stopPropagation();
    if (!edit) return;
    box.setSelectionRange(edit.from, edit.to);
    // insertText keeps Ctrl+Z working and fires the input event the box already listens to.
    const done = typeof document.execCommand === 'function' && document.execCommand('insertText', false, edit.insert);
    if (!done) {
      box.setRangeText(edit.insert, edit.from, edit.to, 'end');
      box.dispatchEvent(new Event('input', { bubbles: true }));
    }
    box.setSelectionRange(edit.start, edit.end);
  }
  function onblur() {
    leaving = false;
  }
  box.addEventListener('keydown', onkeydown);
  box.addEventListener('blur', onblur);
  return {
    destroy() {
      box.removeEventListener('keydown', onkeydown);
      box.removeEventListener('blur', onblur);
    },
  };
}
