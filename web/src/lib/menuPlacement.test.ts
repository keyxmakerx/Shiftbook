import { describe, expect, it } from 'vitest';
import { growMenu, MARGIN, placeMenu, TUCK, type Box } from './menuPlacement';

const VIEW = { width: 1200, height: 800 };
const SHEET = { width: 252, height: 300 };

function box(left: number, top: number, width: number, height: number): Box {
  return { left, top, width, height };
}

describe('paper menu placement', () => {
  it('pulls the sheet out below the object, lined up with its right edge', () => {
    const row = box(100, 200, 600, 44);
    const p = placeMenu(row, VIEW, SHEET);
    expect(p.sheetFrom).toBe('below');
    expect(p.sheetY).toBe(244 - TUCK);
    expect(p.sheetX + SHEET.width).toBe(700);
    expect(p.sheetHeight).toBe(SHEET.height);
  });

  it('keeps a sheet under a small object near the left edge inside the window', () => {
    const chip = box(8, 300, 80, 20);
    const p = placeMenu(chip, VIEW, SHEET);
    expect(p.sheetX).toBe(MARGIN);
  });

  it('pulls the sheet out upward near the bottom of the window', () => {
    const row = box(100, 700, 600, 44);
    const p = placeMenu(row, VIEW, SHEET);
    expect(p.sheetFrom).toBe('above');
    expect(p.sheetY + SHEET.height).toBe(700 + TUCK);
  });

  it('shortens a sheet that does not fit so it still starts at the object', () => {
    const tall = { width: 252, height: 640 };
    const row = box(100, 520, 600, 44);
    const p = placeMenu(row, VIEW, tall);
    expect(p.sheetFrom).toBe('above');
    expect(p.sheetHeight).toBe(520 + TUCK - MARGIN);
    expect(p.sheetY).toBe(MARGIN);
    // Enough room below: it still comes out below, shorter.
    const high = placeMenu(box(100, 300, 600, 44), VIEW, tall);
    expect(high.sheetFrom).toBe('below');
    expect(high.sheetY).toBe(344 - TUCK);
    expect(high.sheetHeight).toBe(VIEW.height - MARGIN - (344 - TUCK));
  });

  it('grows down when a row opens its choices, then up, keeping the rows in place', () => {
    // Below with room to spare: it grows down and its top stays.
    const first = placeMenu(box(100, 200, 600, 44), VIEW, SHEET);
    const grown = growMenu(first, VIEW, 420);
    expect(grown.sheetY).toBe(first.sheetY);
    expect(grown.sheetHeight).toBe(420);
    expect(grown.sheetFrom).toBe('below');
    // Closed again: back where it came out.
    expect(growMenu(first, VIEW, SHEET.height)).toEqual(first);
    // Out of room below: it moves up as far as it needs to, over the object.
    const low = placeMenu(box(100, 420, 600, 44), VIEW, SHEET);
    const up = growMenu(low, VIEW, 500);
    expect(up.sheetHeight).toBe(500);
    expect(up.sheetY + up.sheetHeight).toBe(VIEW.height - MARGIN);
    // Came out above, cramped at the top: it grows down over the object instead of scrolling.
    const above = placeMenu(box(100, 160, 600, 600), VIEW, { width: 252, height: 160 });
    expect(above.sheetFrom).toBe('above');
    const colours = growMenu(above, VIEW, 240);
    expect(colours.sheetY).toBe(above.sheetY);
    expect(colours.sheetHeight).toBe(240);
    // One that came out shortened (scrolling) goes back to that when the row closes.
    const cramped = placeMenu(box(100, 380, 600, 44), VIEW, { width: 252, height: 395 });
    expect(cramped.sheetHeight).toBeLessThan(395);
    expect(growMenu(cramped, VIEW, 395, 395)).toEqual(cramped);
    expect(growMenu(cramped, VIEW, 480, 395).sheetHeight).toBe(480);
    // Taller than the window: it fills it and scrolls.
    const tall = growMenu(first, VIEW, 2000);
    expect(tall.sheetY).toBe(MARGIN);
    expect(tall.sheetHeight).toBe(VIEW.height - 2 * MARGIN);
  });

  it('keeps the sheet inside the window', () => {
    for (const left of [0, 40, 600, 1100, 1190]) {
      for (const top of [0, 100, 400, 790]) {
        const p = placeMenu(box(left, top, 10, 10), VIEW, SHEET);
        expect(p.sheetX).toBeGreaterThanOrEqual(MARGIN);
        expect(p.sheetX + SHEET.width).toBeLessThanOrEqual(VIEW.width - MARGIN);
        expect(p.sheetY).toBeGreaterThanOrEqual(MARGIN);
        expect(p.sheetY + p.sheetHeight).toBeLessThanOrEqual(VIEW.height - MARGIN);
      }
    }
  });
});
