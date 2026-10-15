import { describe, expect, it } from 'vitest';
import { GAP, MARGIN, placeMenu, TUCK, type Box } from './menuPlacement';

const VIEW = { width: 1200, height: 800 };
const SHEET = { width: 252, height: 300 };
const SIDE = { width: 36, height: 120 };
const TOP = { width: 120, height: 36 };

function box(left: number, top: number, width: number, height: number): Box {
  return { left, top, width, height };
}

function overlap(a: Box, b: Box): boolean {
  return a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height;
}

describe('paper menu placement', () => {
  it('puts the rail on the right, the top rail above and the sheet below the object', () => {
    const row = box(100, 200, 600, 44);
    const p = placeMenu(row, VIEW, SHEET, SIDE, TOP);
    expect(p.side).toBe('right');
    expect(p.sideX).toBe(700 + GAP);
    expect(p.sideY).toBe(200);
    expect(p.topWhere).toBe('above');
    expect(p.topY).toBe(200 - GAP - TOP.height);
    expect(p.topX).toBe(100);
    expect(p.sheetFrom).toBe('below');
    expect(p.sheetY).toBe(244 - TUCK);
    // Lined up with the object's right edge, next to the rail.
    expect(p.sheetX + SHEET.width).toBe(700);
  });

  it('uses the left side when there is no room on the right', () => {
    const chip = box(1120, 300, 70, 20);
    const p = placeMenu(chip, VIEW, SHEET, SIDE, TOP);
    expect(p.side).toBe('left');
    expect(p.sideX).toBe(1120 - GAP - SIDE.width);
    // The sheet can't line up under the chip without covering the rail, so it sits beside it.
    expect(p.sheetX).toBe(p.sideX - GAP - SHEET.width);
  });

  it('never lets the rail cover the sheet', () => {
    for (const left of [8, 40, 300, 900, 1100]) {
      for (const top of [10, 100, 400, 700]) {
        const anchor = box(left, top, 80, 20);
        const p = placeMenu(anchor, VIEW, SHEET, SIDE, TOP);
        const rail = box(p.sideX, p.sideY, SIDE.width, SIDE.height);
        const sheet = box(p.sheetX, p.sheetY, SHEET.width, SHEET.height);
        expect(overlap(rail, sheet), `${left},${top}`).toBe(false);
      }
    }
  });

  it('pulls the sheet out upward near the bottom of the window', () => {
    const row = box(100, 700, 600, 44);
    const p = placeMenu(row, VIEW, SHEET, SIDE, TOP);
    expect(p.sheetFrom).toBe('above');
    expect(p.sheetY + SHEET.height).toBe(700 + TUCK);
    // The top rail moves below the object so it doesn't collide with the sheet.
    expect(p.topWhere).toBe('below');
    expect(p.topY).toBe(Math.min(744 + GAP, VIEW.height - MARGIN - TOP.height));
  });

  it('moves the top rail under the sheet when there is no room above', () => {
    const day = box(400, 20, 24, 50);
    const p = placeMenu(day, VIEW, SHEET, SIDE, TOP);
    expect(p.sheetFrom).toBe('below');
    expect(p.topWhere).toBe('below');
    expect(p.topY).toBe(p.sheetY + SHEET.height + GAP);
  });

  it('keeps everything inside the window', () => {
    const p = placeMenu(box(0, 0, 1200, 800), VIEW, SHEET, SIDE, TOP);
    for (const [x, w] of [
      [p.sideX, SIDE.width],
      [p.topX, TOP.width],
      [p.sheetX, SHEET.width],
    ] as const) {
      expect(x).toBeGreaterThanOrEqual(MARGIN);
      expect(x + w).toBeLessThanOrEqual(VIEW.width - MARGIN);
    }
    expect(p.sheetY).toBeGreaterThanOrEqual(MARGIN);
    expect(p.sheetY + SHEET.height).toBeLessThanOrEqual(VIEW.height - MARGIN);
  });
});
