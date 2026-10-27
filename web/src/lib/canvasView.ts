// Pan and zoom for the Desk (the Canvas). Cards live in canvas units; the screen shows them at
// `zoom`, shifted by `x`/`y` pixels: screen = canvas × zoom + offset. Pure maths, tested.

import { CARD_H, CARD_W, GAP, MIN_AREA } from './api/generated/constants';

export interface CanvasView {
  x: number;
  y: number;
  zoom: number;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 2;
/** Card size in canvas units, from core (`CARD_W`/`CARD_H` in crates/core/src/canvas.rs). */
export { CARD_H, CARD_W, MIN_AREA, NEW_AREA } from './api/generated/constants';

export const HOME: CanvasView = { x: 0, y: 0, zoom: 1 };
/** The view the Desk opens with: shifted right so the first cards clear the toolbox and the Drawers button. */
export const START: CanvasView = { x: 104, y: 0, zoom: 1 };

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

/** Where a screen point (relative to the board) is in canvas units. */
export function toCanvas(view: CanvasView, sx: number, sy: number): { x: number; y: number } {
  return { x: (sx - view.x) / view.zoom, y: (sy - view.y) / view.zoom };
}

/** Zoom by `factor`, keeping the canvas point under (sx, sy) where it is on screen. */
export function zoomAt(view: CanvasView, sx: number, sy: number, factor: number): CanvasView {
  const zoom = clampZoom(view.zoom * factor);
  const at = toCanvas(view, sx, sy);
  return { zoom, x: sx - at.x * zoom, y: sy - at.y * zoom };
}

/** The view that shows every card in a `width` × `height` board, with a margin; never zooms past 100%. */
export function fitAll(cards: { x: number; y: number }[], width: number, height: number, margin = 40): CanvasView {
  if (cards.length === 0 || width <= 0 || height <= 0) return HOME;
  const left = Math.min(...cards.map((c) => c.x));
  const top = Math.min(...cards.map((c) => c.y));
  const right = Math.max(...cards.map((c) => c.x + CARD_W));
  const bottom = Math.max(...cards.map((c) => c.y + CARD_H));
  const zoom = clampZoom(Math.min(1, (width - 2 * margin) / (right - left), (height - 2 * margin) / (bottom - top)));
  return {
    zoom,
    x: (width - (right - left) * zoom) / 2 - left * zoom,
    y: (height - (bottom - top) * zoom) / 2 - top * zoom,
  };
}


import type { BoxStatus, CanvasArea, CanvasLabel } from './api';

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The index of the area holding a card's centre (the last one wins), as `area_at` in core. */
export function areaAt(areas: Rect[], x: number, y: number): number {
  const cx = x + CARD_W / 2;
  const cy = y + CARD_H / 2;
  for (let i = areas.length - 1; i >= 0; i--) {
    const a = areas[i]!;
    if (cx >= a.x && cx < a.x + a.w && cy >= a.y && cy < a.y + a.h) return i;
  }
  return -1;
}

/** A box on the Desk: a project's area or a plain labelled box (round 12). */
export interface DeskBox extends Rect {
  /** "project:<id>" or "label:<id>", as cards' `inBox`. */
  key: string;
  projectId: string | null;
  labelId: string | null;
  /** The label of a labelled box (a project box shows the project's name). */
  label: string;
  collapsed: boolean;
  count: number;
  status: BoxStatus;
}

/** Every box, project areas first then labelled boxes: the order core counts cards in. */
export function deskBoxes(areas: CanvasArea[], labels: CanvasLabel[]): DeskBox[] {
  return [
    ...areas.map((a) => ({ ...a, key: `project:${a.projectId}`, labelId: null, label: '' })),
    ...labels.map((l) => ({ ...l, key: `label:${l.id}`, projectId: null, labelId: l.id })),
  ];
}

/** A rectangle from two corners, in either order. */
export function rectFrom(a: { x: number; y: number }, b: { x: number; y: number }): Rect {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y) };
}

/** True when a card at (x, y) has its centre inside `r` (how boxes and selections pick cards). */
export function cardIn(r: Rect, x: number, y: number): boolean {
  const cx = x + CARD_W / 2;
  const cy = y + CARD_H / 2;
  return cx >= r.x && cx < r.x + r.w && cy >= r.y && cy < r.y + r.h;
}

/** A box just big enough around some cards, with room for its name tab. */
export function boxAround(spots: { x: number; y: number }[], pad = 24): Rect {
  const xs = spots.map((s) => s.x);
  const ys = spots.map((s) => s.y);
  const x = Math.min(...xs) - pad;
  const y = Math.min(...ys) - pad - 12;
  return {
    x,
    y,
    w: Math.max(MIN_AREA.w, Math.max(...xs) + CARD_W + pad - x),
    h: Math.max(MIN_AREA.h, Math.max(...ys) + CARD_H + pad - y),
  };
}

/** The card a dragged card was dropped on: the topmost one holding the dragged card's centre
 *  (the last in `spots`), never `skip`. Mirrors `card_under` in crates/core/src/canvas.rs. */
export function cardUnder(spots: { x: number; y: number }[], x: number, y: number, skip = -1): number {
  const cx = x + CARD_W / 2;
  const cy = y + CARD_H / 2;
  for (let i = spots.length - 1; i >= 0; i--) {
    if (i === skip) continue;
    const s = spots[i]!;
    if (cx >= s.x && cx < s.x + CARD_W && cy >= s.y && cy < s.y + CARD_H) return i;
  }
  return -1;
}

/** Where the cards of a fanned-out pile show: in a row to the right of the pile, wrapping
 *  after `perRow`. */
export function fanSpots(x: number, y: number, count: number, perRow = 4): { x: number; y: number }[] {
  return Array.from({ length: count }, (_, i) => ({
    x: x + (CARD_W + 16) * ((i % perRow) + 1),
    y: y + (CARD_H + 16) * Math.floor(i / perRow),
  }));
}

// ----- lining up (round 15) -----

/** How near, in screen pixels, a card held with Shift has to come to pull into line. */
export const LINE_UP_PX = 12;
/** Space the Line up buttons leave between cards: core's `GAP`. */
export const LINE_UP_GAP = GAP;

/** How far in from a box's sides a card lines up inside it; the top leaves room for the name tab
 *  (as `boxAround` does). */
export const BOX_INSET = { side: 24, top: 36 };

/**
 * A guide line while lining up, in canvas units: `upright` lines run down at x = `at`, `flat`
 * ones run across at y = `at`, from `from` to `to`. `gap` marks one of a run of equal gaps.
 */
export interface Guide {
  dir: 'upright' | 'flat';
  at: number;
  from: number;
  to: number;
  gap?: boolean;
}

type Spot = { x: number; y: number };
type Axis = 'x' | 'y';
type Choice =
  | { v: number; kind: 'edge' }
  | { v: number; kind: 'box'; box: Rect; line: number }
  | { v: number; kind: 'gap'; size: number };

const SIZE = { x: CARD_W, y: CARD_H };
const cross = (a: Axis): Axis => (a === 'x' ? 'y' : 'x');
const boxSize = (b: Rect, a: Axis) => (a === 'x' ? b.w : b.h);

/** Where along one axis a card at `p` could line up, nearest first, and which one wins. */
function pickAlong(a: Axis, p: Spot, others: Spot[], boxes: Rect[], within: number): Choice | null {
  const c = cross(a);
  const size = SIZE[a];
  const options: Choice[] = [];
  // Another card's edge (cards are all one size, so its left, middle and right line up together).
  for (const o of others) options.push({ v: o[a], kind: 'edge' });
  // A box's inside edges and its middle.
  for (const b of boxes) {
    const start = b[a] + (a === 'x' ? BOX_INSET.side : BOX_INSET.top);
    const end = b[a] + boxSize(b, a) - BOX_INSET.side - size;
    options.push({ v: start, kind: 'box', box: b, line: start });
    options.push({ v: end, kind: 'box', box: b, line: end + size });
    const mid = b[a] + (boxSize(b, a) - size) / 2;
    options.push({ v: mid, kind: 'box', box: b, line: mid + size / 2 });
  }
  // Even spacing with the cards in the same row (or column): the usual gap, any gap already
  // between two of them, or halfway between the two either side.
  const band = others.filter((o) => Math.abs(o[c] - p[c]) < SIZE[c]).sort((m, n) => m[a] - n[a]);
  const gaps = new Set([LINE_UP_GAP]);
  for (let i = 1; i < band.length; i++) {
    const g = band[i]![a] - band[i - 1]![a] - size;
    if (g > 0 && g < 3 * size) gaps.add(g);
  }
  const before = band.filter((o) => o[a] + size / 2 <= p[a] + size / 2).at(-1);
  const after = band.find((o) => o[a] + size / 2 > p[a] + size / 2);
  for (const g of gaps) {
    if (before) options.push({ v: before[a] + size + g, kind: 'gap', size: g });
    if (after) options.push({ v: after[a] - size - g, kind: 'gap', size: g });
  }
  if (before && after) {
    const g = (after[a] - before[a] - 2 * size) / 2;
    if (g > 0) options.push({ v: before[a] + size + g, kind: 'gap', size: g });
  }
  let best: Choice | null = null;
  for (const o of options) {
    const d = Math.abs(o.v - p[a]);
    // Lining up with a card beats a box, and a box beats spacing, when they're as near.
    if (d <= within && (!best || d < Math.abs(best.v - p[a]) - 0.5)) best = o;
  }
  return best;
}

/** The guide lines that show why a card at `p` sits where it does along one axis. */
function guidesFor(a: Axis, why: Choice, p: Spot, others: Spot[]): Guide[] {
  const c = cross(a);
  const size = SIZE[a];
  const dir = a === 'x' ? 'upright' : 'flat';
  if (why.kind === 'edge') {
    const crosses = [p[c], ...others.filter((o) => o[a] === why.v).map((o) => o[c])];
    return [{ dir, at: why.v, from: Math.min(...crosses), to: Math.max(...crosses) + SIZE[c] }];
  }
  if (why.kind === 'box') return [{ dir, at: why.line, from: why.box[c], to: why.box[c] + boxSize(why.box, c) }];
  // Equal gaps: a short line across each gap of that size in the row, the card's own included.
  const along = a === 'x' ? 'flat' : 'upright';
  const row = [...others.filter((o) => Math.abs(o[c] - p[c]) < SIZE[c]), p].sort((m, n) => m[a] - n[a]);
  const out: Guide[] = [];
  for (let i = 1; i < row.length; i++) {
    const m = row[i - 1]!;
    const n = row[i]!;
    if (Math.abs(n[a] - m[a] - size - why.size) < 0.5 && (m === p || n === p || out.length < 6)) {
      // Across a row the marks sit near the cards' tops: cards are often shorter than CARD_H.
      const at = a === 'x' ? Math.max(m.y, n.y) + 24 : Math.max(m.x, n.x) + CARD_W / 2;
      out.push({ dir: along, at, from: m[a] + size, to: n[a], gap: true });
    }
  }
  return out;
}

/**
 * Pull a card at (x, y), held with Shift, into line (like the smart guides in drawing apps):
 * onto another card's edges, a box's inside edges or middle, or evenly spaced with the cards
 * in its row or column. Each direction only within `within` canvas units. Returns where it goes
 * and the guide lines that show what it lined up with.
 */
export function lineUpWith(
  x: number,
  y: number,
  others: Spot[],
  boxes: Rect[],
  within: number,
): { x: number; y: number; guides: Guide[] } {
  const p = { x, y };
  const wy = pickAlong('y', p, others, boxes, within);
  if (wy) p.y = wy.v;
  // Across second, so spacing counts the row the card has just lined up with.
  const wx = pickAlong('x', p, others, boxes, within);
  if (wx) p.x = wx.v;
  return {
    ...p,
    guides: [...(wx ? guidesFor('x', wx, p, others) : []), ...(wy ? guidesFor('y', wy, p, others) : [])],
  };
}

/**
 * The Line up buttons: new spots for `spots`, in the same order. A row keeps left-to-right
 * order along the top; a column keeps top-to-bottom order down the left edge; a grid fills
 * rows in reading order, about as many across as down. All start at the top left of the cards.
 */
export function lineUp(spots: { x: number; y: number }[], how: 'row' | 'column' | 'grid'): { x: number; y: number }[] {
  if (!spots.length) return [];
  const left = Math.min(...spots.map((s) => s.x));
  const top = Math.min(...spots.map((s) => s.y));
  const across = how === 'row' ? spots.length : how === 'column' ? 1 : Math.ceil(Math.sqrt(spots.length));
  const index = spots.map((_, i) => i);
  if (how === 'row') index.sort((a, b) => spots[a]!.x - spots[b]!.x || spots[a]!.y - spots[b]!.y);
  else index.sort((a, b) => spots[a]!.y - spots[b]!.y || spots[a]!.x - spots[b]!.x);
  if (how === 'grid') {
    // Reading order: a card whose top is within half a card of the line's first card is on that line.
    const lines: number[][] = [];
    for (const i of index) {
      const line = lines.at(-1);
      if (line && spots[i]!.y - spots[line[0]!]!.y < CARD_H / 2) line.push(i);
      else lines.push([i]);
    }
    index.splice(0, index.length, ...lines.flatMap((l) => l.sort((a, b) => spots[a]!.x - spots[b]!.x)));
  }
  const out = spots.map((s) => ({ ...s }));
  index.forEach((i, n) => {
    out[i] = { x: left + (n % across) * (CARD_W + LINE_UP_GAP), y: top + Math.floor(n / across) * (CARD_H + LINE_UP_GAP) };
  });
  return out;
}

/** The drawer under a screen point while something is dragged over the cabinet. */
export function drawerAt(clientX: number, clientY: number): string | null {
  for (const el of document.elementsFromPoint(clientX, clientY)) {
    const key = (el as HTMLElement).closest<HTMLElement>('[data-drawer]')?.dataset.drawer;
    if (key) return key;
  }
  return null;
}
