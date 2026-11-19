// Find a word inside the open notepad or MOP (round 21, card r21-notepads), and the time
// stamp Ctrl+; drops in. Ctrl+F looks in the notepad the cursor is in, else the window in
// front (a notepad, or a MOP being read or run). The match is shown by a marker drawn over
// it, so it shows in a text box and on a rendered page alike and nothing gets selected or
// changed while you look. The parts that need no page are in lib/findIn.ts.
import { joinPieces, pieceAt, startAt, timeStamp, wrapIndex, type Piece } from './findIn';
import { findAll } from './mop';

/** A marker over part of a match, in window coordinates. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const finder = $state({
  open: false,
  query: '',
  index: 0,
  total: 0,
  boxes: [] as Box[],
  /** Where the bar sits. */
  at: { x: 0, y: 0 },
  /** Goes up each time Ctrl+F is pressed, so the bar takes the cursor again. */
  serial: 0,
});

const BAR = { w: 300, h: 36 };

let target: HTMLElement | null = null;
let text = '';
let pieces: Piece<Text>[] = [];
let matches: number[] = [];
let seen = '';
let frame = 0;

const shown = (el: Element) => (el as HTMLElement).offsetParent !== null;

/** The notepad or MOP that Ctrl+F is for, or null when it isn't clear which. */
function pickTarget(): HTMLElement | null {
  const active = document.activeElement;
  if (active instanceof HTMLTextAreaElement) return active;
  const near = active?.closest?.<HTMLElement>('[data-find]');
  if (near) return near;
  const within = (scope: Element | null) =>
    scope ? [...scope.querySelectorAll<HTMLElement>('[data-find], textarea')].filter(shown) : [];
  const front = within(document.querySelector('.win.front'));
  if (front[0]) return front[0];
  const page = within(document.querySelector('.view'));
  return page.length === 1 ? page[0]! : null;
}

function read(): void {
  if (!target) return;
  if (target instanceof HTMLTextAreaElement) {
    text = target.value;
    pieces = [];
  } else {
    const parts: { node: Text; text: string }[] = [];
    const walk = document.createTreeWalker(target, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n; n = walk.nextNode()) parts.push({ node: n as Text, text: (n as Text).data });
    ({ text, pieces } = joinPieces(parts));
  }
  matches = findAll(text, finder.query);
  finder.total = matches.length;
  finder.index = wrapIndex(finder.index, matches.length);
}

const COPIED = [
  'paddingTop',
  'paddingRight',
  'paddingBottom',
  'paddingLeft',
  'borderTopWidth',
  'borderRightWidth',
  'borderBottomWidth',
  'borderLeftWidth',
  'fontFamily',
  'fontSize',
  'fontWeight',
  'fontStyle',
  'fontVariant',
  'letterSpacing',
  'lineHeight',
  'textTransform',
  'wordSpacing',
  'textIndent',
  'tabSize',
  'whiteSpace',
  'wordBreak',
  'overflowWrap',
  'textAlign',
  'direction',
] as const;

/**
 * Where the characters `from`..`to` of a text box are drawn, measured on a hidden copy of
 * the box laid out the same way: boxes relative to the top left of its scrolled content.
 */
function inTextBox(box: HTMLTextAreaElement, from: number, to: number): Box[] {
  const cs = getComputedStyle(box);
  const copy = document.createElement('div');
  for (const p of COPIED) copy.style[p] = cs[p];
  const edges = parseFloat(cs.borderLeftWidth) + parseFloat(cs.borderRightWidth);
  Object.assign(copy.style, {
    position: 'fixed',
    left: '-10000px',
    top: '0',
    visibility: 'hidden',
    boxSizing: 'border-box',
    borderStyle: 'solid',
    // The box's width without its scroll bar, so lines wrap where they do in the box.
    width: `${box.clientWidth + edges}px`,
    height: 'auto',
    overflow: 'hidden',
  });
  if (cs.whiteSpace !== 'pre') Object.assign(copy.style, { whiteSpace: 'pre-wrap', overflowWrap: 'break-word' });
  const mark = document.createElement('span');
  mark.textContent = box.value.slice(from, to);
  copy.append(box.value.slice(0, from), mark, box.value.slice(to) || ' ');
  document.body.append(copy);
  const origin = copy.getBoundingClientRect();
  const out = [...mark.getClientRects()].map((r) => ({
    x: r.left - origin.left - parseFloat(cs.borderLeftWidth),
    y: r.top - origin.top - parseFloat(cs.borderTopWidth),
    w: r.width,
    h: r.height,
  }));
  copy.remove();
  return out;
}

/** The boxes of the current match in window coordinates; `scroll` brings it into view first. */
function measure(scroll: boolean): Box[] {
  const from = matches[finder.index];
  if (!target || from === undefined) return [];
  const to = from + finder.query.length;
  const frameBox = target.getBoundingClientRect();
  let out: Box[];
  if (target instanceof HTMLTextAreaElement) {
    const box = target;
    const inside = inTextBox(box, from, to);
    const first = inside[0];
    if (scroll && first && (first.y < box.scrollTop || first.y + first.h > box.scrollTop + box.clientHeight)) {
      box.scrollTop = Math.max(0, first.y - box.clientHeight / 2);
    }
    if (scroll && first && (first.x < box.scrollLeft || first.x + first.w > box.scrollLeft + box.clientWidth)) {
      box.scrollLeft = Math.max(0, first.x - box.clientWidth / 2);
    }
    const cs = getComputedStyle(box);
    const left = frameBox.left + parseFloat(cs.borderLeftWidth) - box.scrollLeft;
    const top = frameBox.top + parseFloat(cs.borderTopWidth) - box.scrollTop;
    out = inside.map((b) => ({ ...b, x: b.x + left, y: b.y + top }));
  } else {
    const a = pieceAt(pieces, from);
    const b = pieceAt(pieces, to);
    if (!a || !b || !a.node.isConnected || !b.node.isConnected) return [];
    const range = document.createRange();
    try {
      range.setStart(a.node, a.offset);
      range.setEnd(b.node, b.offset);
    } catch {
      return [];
    }
    if (scroll) {
      const r = range.getBoundingClientRect();
      if (r.top < frameBox.top || r.bottom > frameBox.bottom) a.node.parentElement?.scrollIntoView({ block: 'center' });
    }
    out = [...range.getClientRects()].map((r) => ({ x: r.left, y: r.top, w: r.width, h: r.height }));
  }
  // Only what shows inside the notepad's own frame.
  const now = target.getBoundingClientRect();
  return out.filter((b) => b.w > 0 && b.y + b.h > now.top && b.y < now.bottom && b.x + b.w > now.left && b.x < now.right);
}

function place(): void {
  if (!target) return;
  const r = target.getBoundingClientRect();
  // A small notepad gets the bar just above it, so it covers none of the words; a tall one
  // (a window's notepad, a MOP) gets it in its top right corner, clear of what's above.
  const small = r.height < 160;
  const above = r.top - BAR.h - 4;
  const y = small && above >= 8 ? above : r.top + 6;
  finder.at = {
    x: Math.min(Math.max(8, r.right - BAR.w - (small ? 6 : 18)), window.innerWidth - BAR.w - 8),
    y: Math.min(Math.max(8, y), window.innerHeight - BAR.h - 8),
  };
}

function signature(): string {
  if (!target) return '';
  const r = target.getBoundingClientRect();
  const length = target instanceof HTMLTextAreaElement ? target.value.length : (target.textContent?.length ?? 0);
  return [r.left, r.top, r.width, r.height, target.scrollTop, target.scrollLeft, length, window.innerWidth].join(',');
}

function show(scroll: boolean): void {
  finder.boxes = measure(scroll);
  place();
  seen = signature();
}

/** While the bar is open: follow the notepad when it scrolls, moves, changes or goes away. */
function watch(): void {
  cancelAnimationFrame(frame);
  const tick = () => {
    if (!finder.open) return;
    if (!target || !target.isConnected || !shown(target)) {
      closeFind(false);
      return;
    }
    if (signature() !== seen) {
      read();
      show(false);
    }
    frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);
}

/** Ctrl+F. False when there is no notepad or MOP to look in. */
export function openFind(): boolean {
  const picked = pickTarget();
  if (!picked) return false;
  const fresh = picked !== target || !finder.open;
  target = picked;
  if (fresh) {
    // A word selected in the notepad is what to look for.
    if (picked instanceof HTMLTextAreaElement) {
      const sel = picked.value.slice(picked.selectionStart, picked.selectionEnd);
      if (sel && sel.length <= 80 && !sel.includes('\n')) finder.query = sel;
    }
    finder.index = 0;
    read();
    if (picked instanceof HTMLTextAreaElement) finder.index = startAt(matches, picked.selectionStart);
  }
  finder.open = true;
  finder.serial += 1;
  show(true);
  watch();
  return true;
}

export function setFindQuery(query: string): void {
  // Stay near the match that was showing rather than jumping back to the top.
  const from = matches[finder.index] ?? 0;
  finder.query = query;
  read();
  finder.index = startAt(matches, from);
  show(true);
}

export function stepFind(by: 1 | -1): void {
  if (!matches.length) return;
  finder.index = wrapIndex(finder.index + by, matches.length);
  show(true);
}

/** Close the bar. `back` puts the cursor in the notepad, on the match in a text box. */
export function closeFind(back = true): void {
  cancelAnimationFrame(frame);
  const from = matches[finder.index];
  const box = target;
  finder.open = false;
  finder.boxes = [];
  target = null;
  if (!back || !box?.isConnected) return;
  if (box instanceof HTMLTextAreaElement) {
    box.focus({ preventScroll: true });
    if (from !== undefined && finder.query) box.setSelectionRange(from, from + finder.query.length);
  }
}

/** Ctrl+; in a notepad: the time, then a space, where the cursor is. Ctrl+Z takes it out. */
export function stampTime(box: HTMLTextAreaElement, now = new Date()): void {
  const stamp = `${timeStamp(now)} `;
  const done = typeof document.execCommand === 'function' && document.execCommand('insertText', false, stamp);
  if (!done) {
    box.setRangeText(stamp, box.selectionStart, box.selectionEnd, 'end');
    box.dispatchEvent(new Event('input', { bubbles: true }));
  }
}
