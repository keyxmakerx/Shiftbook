// Find a word inside the open notepad or MOP (round 21, card r21-notepads): the parts that
// need no page. Where the matches are comes from `findAll` in lib/mop.ts; this is which one
// is current, the time stamp Ctrl+; drops in, and the text of a rendered page.

/** `i` wrapped into 0..total-1 (0 when there is nothing). */
export function wrapIndex(i: number, total: number): number {
  return total <= 0 ? 0 : ((i % total) + total) % total;
}

/** The first match at or after `caret`, else the first one: where Find starts. */
export function startAt(matches: number[], caret: number): number {
  const i = matches.findIndex((m) => m >= caret);
  return i < 0 ? 0 : i;
}

/** "2 of 7", "No matches", or '' before anything is typed. */
export function countWords(query: string, index: number, total: number): string {
  if (!query) return '';
  return total === 0 ? 'No matches' : `${index + 1} of ${total}`;
}

/** The time as it is dropped into a notepad: 24-hour, like 14:05. */
export function timeStamp(now: Date): string {
  return `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`;
}

/** A piece of a rendered page's text: where it starts in the joined text. */
export interface Piece<N> {
  node: N;
  start: number;
  length: number;
}

/**
 * Join the text of a page's pieces (its text nodes) with a line break between them, so a
 * word is never found across two separate lines, and remember where each piece starts.
 */
export function joinPieces<N>(parts: { node: N; text: string }[]): { text: string; pieces: Piece<N>[] } {
  const pieces: Piece<N>[] = [];
  let text = '';
  for (const p of parts) {
    if (!p.text) continue;
    if (text) text += '\n';
    pieces.push({ node: p.node, start: text.length, length: p.text.length });
    text += p.text;
  }
  return { text, pieces };
}

/** The piece holding offset `at` of the joined text, and the offset inside it. */
export function pieceAt<N>(pieces: Piece<N>[], at: number): { node: N; offset: number } | null {
  for (const p of pieces) {
    if (at >= p.start && at <= p.start + p.length) return { node: p.node, offset: at - p.start };
  }
  return null;
}
