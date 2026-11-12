// Where the paper menu's sheet goes around the object it was opened on (pure, tested).
//
// The sheet with the options pulls out from under the object's bottom edge, or its top edge
// when below is cramped and there's more room above; a sheet that doesn't fit gets shorter
// and scrolls. It lines up with the object's right edge, kept inside the window. Since the
// daily-use review (10 Nov) there are no icon rails beside it to keep clear of, and a row can
// open its own choices in place, which makes the sheet longer (`growMenu`).

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

export const MARGIN = 8;
/** How far the sheet starts tucked under the object's edge. */
export const TUCK = 6;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, Math.max(min, max)));
}

/** Place the sheet for `anchor`. */
export function placeMenu(anchor: Box, viewport: Size, sheet: Size): Placement {
  const right = anchor.left + anchor.width;
  const bottom = anchor.top + anchor.height;
  const roomBelow = viewport.height - MARGIN - (bottom - TUCK);
  const roomAbove = anchor.top + TUCK - MARGIN;
  // Below is the natural way out; go above only when below is cramped and above has more.
  const sheetFrom: 'below' | 'above' =
    roomBelow >= sheet.height || roomBelow >= ENOUGH_BELOW || roomBelow >= roomAbove ? 'below' : 'above';
  // Shorten (and scroll) a sheet that doesn't fit, so it still starts at the object's edge.
  const room = sheetFrom === 'below' ? roomBelow : roomAbove;
  const sheetHeight = Math.min(sheet.height, Math.max(room, MIN_SHEET));
  const sheetY = clamp(
    sheetFrom === 'below' ? bottom - TUCK : anchor.top + TUCK - sheetHeight,
    MARGIN,
    viewport.height - MARGIN - sheetHeight,
  );
  const sheetX = clamp(right - sheet.width, MARGIN, viewport.width - MARGIN - sheet.width);
  return { sheetX, sheetY, sheetFrom, sheetHeight };
}

/**
 * The sheet once a row has opened its choices in place and it needs `height`: it keeps its top
 * where it is, so the rows stay put, and grows down as far as the window allows, then up,
 * over the object if it has to. Only a sheet taller than the window scrolls. With the row
 * closed again (no taller than the `asked` height it came out with) it goes back to `first`.
 */
export function growMenu(first: Placement, viewport: Size, height: number, asked = first.sheetHeight): Placement {
  if (height <= Math.max(asked, first.sheetHeight)) return first;
  const sheetHeight = Math.min(height, viewport.height - 2 * MARGIN);
  const sheetY = Math.max(MARGIN, Math.min(first.sheetY, viewport.height - MARGIN - sheetHeight));
  return { ...first, sheetY, sheetHeight };
}
