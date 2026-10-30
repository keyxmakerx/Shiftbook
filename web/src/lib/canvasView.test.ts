import { describe, expect, it } from 'vitest';
import { areaAt, boxAround, cardUnder, fanSpots, CARD_H, CARD_W, cardIn, deskBoxes, fitAll, BOX_INSET, HOME, LINE_UP_GAP, lineUp, lineUpWith, MAX_ZOOM, MIN_ZOOM, OPENING_MIN_ZOOM, openingView, rectFrom, START, toCanvas, zoomAt } from './canvasView';

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

  it('opens on the middle of everything on the Desk', () => {
    expect(openingView([], 800, 600)).toEqual(START);
    // Far from the start, and fits at 100%: centred.
    const far = openingView([{ x: 3000, y: 2000 }], 800, 600);
    expect(far.zoom).toBe(1);
    expect(3000 + far.x).toBeCloseTo((800 - CARD_W) / 2);
    expect(2000 + far.y).toBeCloseTo((600 - CARD_H) / 2);
    // Too spread out to fit at a readable zoom: the middle of it, at that zoom.
    const spread = openingView([{ x: 0, y: 0 }, { x: 6000, y: 0 }], 800, 600);
    expect(spread.zoom).toBe(OPENING_MIN_ZOOM);
    const middle = (6000 + CARD_W) / 2;
    expect(middle * spread.zoom + spread.x).toBeCloseTo(400);
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

  it('pulls a card held with Shift into line with nearby cards', () => {
    const others = [
      { x: 0, y: 0 },
      { x: 300, y: 0 },
      { x: 0, y: 200 },
    ];
    // Near the top of the row and the left edge of the column: both lines.
    const both = lineUpWith(8, 6, others, [], 12);
    expect({ x: both.x, y: both.y }).toEqual({ x: 0, y: 0 });
    expect(both.guides).toContainEqual({ dir: 'upright', at: 0, from: 0, to: 200 + CARD_H });
    expect(both.guides).toContainEqual({ dir: 'flat', at: 0, from: 0, to: 300 + CARD_W });
    // Too far from everything: it stays where it is.
    expect(lineUpWith(150, 100, others, [], 12)).toEqual({ x: 150, y: 100, guides: [] });
    // The nearest edge wins.
    expect(lineUpWith(297, 500, [{ x: 290, y: 0 }, { x: 300, y: 0 }], [], 12).x).toBe(300);
  });

  it('spaces a card evenly with its row', () => {
    // Two cards 40 apart; the third lands 40 after the second, and both gaps show.
    const row = [
      { x: 0, y: 0 },
      { x: CARD_W + 40, y: 0 },
    ];
    const next = lineUpWith(2 * (CARD_W + 40) + 7, 5, row, [], 12);
    expect({ x: next.x, y: next.y }).toEqual({ x: 2 * (CARD_W + 40), y: 0 });
    const gaps = next.guides.filter((g) => g.gap);
    expect(gaps).toHaveLength(2);
    expect(gaps.every((g) => g.to - g.from === 40)).toBe(true);
    // Halfway between two cards is a choice too.
    const wide = [
      { x: 0, y: 0 },
      { x: 1000, y: 0 },
    ];
    expect(lineUpWith(495, 0, wide, [], 12).x).toBe(500);
    // A card in another row doesn't count.
    expect(lineUpWith(CARD_W + 30, 400, [{ x: 0, y: 0 }], [], 12).x).toBe(CARD_W + 30);
  });

  it('lines a card up inside a drawn box', () => {
    const box = { x: 100, y: 100, w: 600, h: 400 };
    const inside = lineUpWith(130, 130, [], [box], 12);
    expect({ x: inside.x, y: inside.y }).toEqual({ x: 100 + BOX_INSET.side, y: 100 + BOX_INSET.top });
    expect(inside.guides).toContainEqual({ dir: 'upright', at: 100 + BOX_INSET.side, from: 100, to: 500 });
    // The right side inside, and the middle.
    expect(lineUpWith(700 - BOX_INSET.side - CARD_W - 5, 300, [], [box], 12).x).toBe(700 - BOX_INSET.side - CARD_W);
    const mid = lineUpWith(400 - CARD_W / 2 + 4, 300, [], [box], 12);
    expect(mid.x).toBe(400 - CARD_W / 2);
    expect(mid.guides).toContainEqual({ dir: 'upright', at: 400, from: 100, to: 500 });
    // A card's edge wins over a box's when they're as near.
    expect(lineUpWith(126, 600, [{ x: 126, y: 0 }], [box], 12).x).toBe(126);
  });

  it('lines selected cards up in a row, a column or a grid', () => {
    const step = { x: CARD_W + LINE_UP_GAP, y: CARD_H + LINE_UP_GAP };
    const spots = [
      { x: 400, y: 90 },
      { x: 50, y: 120 },
      { x: 200, y: 40 },
    ];
    // Same order in, new spots out: left to right along the top.
    expect(lineUp(spots, 'row')).toEqual([
      { x: 50 + 2 * step.x, y: 40 },
      { x: 50, y: 40 },
      { x: 50 + step.x, y: 40 },
    ]);
    // Top to bottom down the left edge.
    expect(lineUp(spots, 'column')).toEqual([
      { x: 50, y: 40 + step.y },
      { x: 50, y: 40 + 2 * step.y },
      { x: 50, y: 40 },
    ]);
    // Four cards make two by two, read like a page: cards with near tops share a line.
    const four = [
      { x: 500, y: 10 },
      { x: 0, y: 300 },
      { x: 100, y: 0 },
      { x: 400, y: 320 },
    ];
    expect(lineUp(four, 'grid')).toEqual([
      { x: step.x, y: 0 },
      { x: 0, y: step.y },
      { x: 0, y: 0 },
      { x: step.x, y: step.y },
    ]);
    expect(lineUp([], 'row')).toEqual([]);
  });
});
