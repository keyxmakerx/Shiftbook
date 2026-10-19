import { describe, expect, it } from 'vitest';
import { areaAt, CARD_H, CARD_W, fitAll, HOME, MAX_ZOOM, MIN_ZOOM, toCanvas, zoomAt } from './canvasView';

describe('canvas view', () => {
  it('maps screen points to canvas units', () => {
    expect(toCanvas({ x: 100, y: 50, zoom: 2 }, 300, 250)).toEqual({ x: 100, y: 100 });
    expect(toCanvas(HOME, 10, 20)).toEqual({ x: 10, y: 20 });
  });

  it('zooms around the pointer and keeps it still', () => {
    const view = { x: 30, y: -20, zoom: 1 };
    const before = toCanvas(view, 400, 300);
    const after = zoomAt(view, 400, 300, 1.5);
    expect(after.zoom).toBe(1.5);
    const still = toCanvas(after, 400, 300);
    expect(still.x).toBeCloseTo(before.x);
    expect(still.y).toBeCloseTo(before.y);
  });

  it('keeps zoom within limits', () => {
    expect(zoomAt(HOME, 0, 0, 100).zoom).toBe(MAX_ZOOM);
    expect(zoomAt(HOME, 0, 0, 0.001).zoom).toBe(MIN_ZOOM);
  });

  it('fits every card with a margin and never zooms in past 100%', () => {
    expect(fitAll([], 800, 600)).toEqual(HOME);
    const one = fitAll([{ x: 0, y: 0 }], 800, 600);
    expect(one.zoom).toBe(1);
    expect(one.x).toBeCloseTo((800 - CARD_W) / 2);
    expect(one.y).toBeCloseTo((600 - CARD_H) / 2);
    const wide = fitAll([{ x: 0, y: 0 }, { x: 2000, y: 0 }], 800, 600);
    expect(wide.zoom).toBeLessThan(1);
    // The right edge of the far card lands inside the margin.
    expect(2000 * wide.zoom + CARD_W * wide.zoom + wide.x).toBeLessThanOrEqual(800 - 40 + 0.01);
  });

  it('finds the area holding a card by its centre, top area first', () => {
    const a = { x: 0, y: 0, w: 600, h: 400 };
    const b = { x: 300, y: 0, w: 600, h: 400 };
    expect(areaAt([a, b], 0, 0)).toBe(0);
    expect(areaAt([a, b], 400, 10)).toBe(1);
    expect(areaAt([a], 520, 0)).toBe(-1);
    expect(areaAt([], 0, 0)).toBe(-1);
  });
});
