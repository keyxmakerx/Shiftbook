import { describe, expect, it } from 'vitest';
import { areaAt, boxAround, cardUnder, fanSpots, CARD_H, CARD_W, cardIn, deskBoxes, fitAll, HOME, MAX_ZOOM, MIN_ZOOM, rectFrom, toCanvas, zoomAt } from './canvasView';

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

describe('boxes and selections', () => {
  it('lists project areas before labelled boxes, keyed like cards', () => {
    const boxes = deskBoxes(
      [{ projectId: 'p1', x: 0, y: 0, w: 300, h: 200, collapsed: false, count: 2, status: 'today' }],
      [{ id: 'l1', label: 'Vendors', x: 400, y: 0, w: 300, h: 200, collapsed: true, count: 0, status: 'quiet' }],
    );
    expect(boxes.map((b) => b.key)).toEqual(['project:p1', 'label:l1']);
    expect(boxes[1]!.projectId).toBeNull();
    expect(boxes[1]!.labelId).toBe('l1');
  });

  it('takes a rectangle from two corners either way round', () => {
    expect(rectFrom({ x: 50, y: 80 }, { x: 10, y: 20 })).toEqual({ x: 10, y: 20, w: 40, h: 60 });
  });

  it('picks cards by their centre', () => {
    const r = { x: 0, y: 0, w: CARD_W, h: CARD_H };
    expect(cardIn(r, 0, 0)).toBe(true);
    expect(cardIn(r, CARD_W / 2, 0)).toBe(false);
  });

  it('draws a box around cards with room for its name', () => {
    const r = boxAround([
      { x: 100, y: 100 },
      { x: 400, y: 300 },
    ]);
    expect(r.x).toBe(76);
    expect(r.y).toBe(64);
    expect(r.x + r.w).toBe(400 + CARD_W + 24);
    expect(r.y + r.h).toBe(300 + CARD_H + 24);
    expect(boxAround([{ x: 0, y: 0 }]).w).toBeGreaterThanOrEqual(CARD_W + 48);
  });
});

describe('piles', () => {
  it('drops on the top card under the centre, never itself', () => {
    const spots = [
      { x: 0, y: 0 },
      { x: 100, y: 50 },
      { x: 1000, y: 1000 },
    ];
    expect(cardUnder(spots, 60, 40, 2)).toBe(1);
    expect(cardUnder(spots, -150, 0, 2)).toBe(-1);
    expect(cardUnder(spots, 1000, 1000, 2)).toBe(-1);
  });

  it('fans a pile out in rows to its right', () => {
    const f = fanSpots(0, 0, 5, 4);
    expect(f[0]).toEqual({ x: CARD_W + 16, y: 0 });
    expect(f[4]).toEqual({ x: CARD_W + 16, y: CARD_H + 16 });
  });
});
