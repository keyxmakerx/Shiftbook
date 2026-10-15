// Where the paper menu's pieces go around the object it was opened on (pure, tested).
//
// - The side rail of icon buttons sits along the object's right side, or its left side
//   when there isn't room on the right (whichever has more room if neither fits).
// - The top rail sits along the object's top; when there's no room above it goes below
//   (under the sheet when the sheet is below).
// - The sheet with the full options pulls out from under the object's bottom edge, or its
//   top edge when below is cramped and there's more room above; a sheet that doesn't fit
//   gets shorter and scrolls. It lines up with the object's edge next to the
//   side rail so the two never overlap; if clamping into the window would make them
//   overlap anyway, the side rail moves to the other side.

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Placement {
  side: 'right' | 'left';
  sideX: number;
  sideY: number;
  topX: number;
  topY: number;
  topWhere: 'above' | 'below';
  sheetX: number;
  sheetY: number;
  /** The sheet comes out from under the object's bottom ('below') or top ('above') edge. */
  sheetFrom: 'below' | 'above';
  /** The sheet's height; less than asked when it has to scroll to fit. */
  sheetHeight: number;
}

/** A sheet never gets shorter than this; below it, it may cover the object instead. */
export const MIN_SHEET = 160;
/** With this much room below the object, the sheet comes out below (scrolling if needed). */
export const ENOUGH_BELOW = 280;

export const GAP = 8;
export const MARGIN = 8;
/** How far the sheet starts tucked under the object's edge. */
export const TUCK = 6;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, Math.max(min, max)));
}

function overlaps(a: Box, b: Box): boolean {
  return (
    a.left < b.left + b.width &&
    b.left < a.left + a.width &&
    a.top < b.top + b.height &&
    b.top < a.top + a.height
  );
}

export function placeMenu(
  anchor: Box,
  viewport: Size,
  sheet: Size,
  sideRail: Size,
  topRail: Size,
): Placement {
  const right = anchor.left + anchor.width;
  const bottom = anchor.top + anchor.height;
  const roomRight = viewport.width - MARGIN - (right + GAP);
  const roomLeft = anchor.left - GAP - MARGIN;
  const fitsRight = roomRight >= sideRail.width;
  const fitsLeft = roomLeft >= sideRail.width;
  let side: 'right' | 'left' = fitsRight || (!fitsLeft && roomRight >= roomLeft) ? 'right' : 'left';

  const roomBelow = viewport.height - MARGIN - (bottom - TUCK);
  const roomAbove = anchor.top + TUCK - MARGIN;
  // Below is the natural way out; go above only when below is cramped and above has more.
  const sheetFrom: 'below' | 'above' =
    roomBelow >= sheet.height || roomBelow >= ENOUGH_BELOW || roomBelow >= roomAbove
      ? 'below'
      : 'above';
  // Shorten (and scroll) a sheet that doesn't fit, so it still starts at the object's edge.
  const room = sheetFrom === 'below' ? roomBelow : roomAbove;
  const sheetHeight = Math.min(sheet.height, Math.max(room, MIN_SHEET));
  const sheetY = clamp(
    sheetFrom === 'below' ? bottom - TUCK : anchor.top + TUCK - sheetHeight,
    MARGIN,
    viewport.height - MARGIN - sheetHeight,
  );

  const sideY = clamp(anchor.top, MARGIN, viewport.height - MARGIN - sideRail.height);
  const place = (s: 'right' | 'left') => {
    const sideX = s === 'right' ? right + GAP : anchor.left - GAP - sideRail.width;
    const wantX = s === 'right' ? right - sheet.width : anchor.left;
    const sheetX = clamp(wantX, MARGIN, viewport.width - MARGIN - sheet.width);
    return { sideX: clamp(sideX, MARGIN, viewport.width - MARGIN - sideRail.width), sheetX };
  };
  let { sideX, sheetX } = place(side);
  const railBox = (x: number): Box => ({ left: x, top: sideY, ...wh(sideRail) });
  const sheetBox = (x: number): Box => ({ left: x, top: sheetY, width: sheet.width, height: sheetHeight });
  if (overlaps(railBox(sideX), sheetBox(sheetX))) {
    const other = side === 'right' ? 'left' : 'right';
    const alt = place(other);
    if (!overlaps(railBox(alt.sideX), sheetBox(alt.sheetX))) {
      side = other;
      ({ sideX, sheetX } = alt);
    } else {
      // Squeezed against the window edge: put the sheet beside the rail instead.
      const beside =
        side === 'right' ? sideX + sideRail.width + GAP : sideX - GAP - sheet.width;
      sheetX = clamp(beside, MARGIN, viewport.width - MARGIN - sheet.width);
    }
  }

  const topX = clamp(anchor.left, MARGIN, viewport.width - MARGIN - topRail.width);
  const aboveY = anchor.top - GAP - topRail.height;
  let topWhere: 'above' | 'below' = 'above';
  let topY = aboveY;
  const sheetAbove = sheetFrom === 'above';
  if (aboveY < MARGIN || sheetAbove) {
    topWhere = 'below';
    topY = sheetAbove ? bottom + GAP : sheetY + sheetHeight + GAP;
    topY = clamp(topY, MARGIN, viewport.height - MARGIN - topRail.height);
  }
  return { side, sideX, sideY, topX, topY, topWhere, sheetX, sheetY, sheetFrom, sheetHeight };
}

function wh(s: Size): { width: number; height: number } {
  return { width: s.width, height: s.height };
}
