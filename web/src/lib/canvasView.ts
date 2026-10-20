// Pan and zoom for the Desk (the Canvas). Cards live in canvas units; the screen shows them at
// `zoom`, shifted by `x`/`y` pixels: screen = canvas × zoom + offset. Pure maths, tested.

export interface CanvasView {
  x: number;
  y: number;
  zoom: number;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 2;
/** Card size in canvas units; matches `CARD_W`/`CARD_H` in crates/core/src/canvas.rs. */
export const CARD_W = 220;
export const CARD_H = 120;

export const HOME: CanvasView = { x: 0, y: 0, zoom: 1 };

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

/** A new project area's size; matches `new_area` in crates/core/src/canvas.rs. */
export const NEW_AREA = { w: 2 * (CARD_W + 24) + 24, h: 2 * (CARD_H + 24) + 48 };
/** Smallest area; matches `MIN_AREA` in crates/core/src/canvas.rs. */
export const MIN_AREA = { w: CARD_W + 48, h: CARD_H + 72 };

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

/** The drawer under a screen point while something is dragged over the cabinet. */
export function drawerAt(clientX: number, clientY: number): string | null {
  for (const el of document.elementsFromPoint(clientX, clientY)) {
    const key = (el as HTMLElement).closest<HTMLElement>('[data-drawer]')?.dataset.drawer;
    if (key) return key;
  }
  return null;
}
