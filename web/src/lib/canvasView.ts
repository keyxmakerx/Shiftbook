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
