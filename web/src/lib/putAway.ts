// Putting cards away (round 15): off the Desk into the Archive drawer, with their hours,
// tickets and notes kept. Entries and done tickets can be put away; notes have their own
// Archive, and open tickets and pinned cards stay put.
import type { CanvasCard, CardRef } from './api';

export const ARCHIVE = 'archive';

/** True when a card can be put away. */
export function canPutAway(card: Pick<CanvasCard, 'kind' | 'tray'>): boolean {
  return card.kind === 'entry' || (card.kind === 'tray' && !!card.tray?.item.done);
}

/** Everything a card holds: an entry card's other days of the same work go with it. */
export function refsOf(card: Pick<CanvasCard, 'kind' | 'id' | 'work'>): CardRef[] {
  return [{ kind: card.kind, id: card.id }, ...(card.work ?? []).map((e) => ({ kind: 'entry' as const, id: e.id }))];
}

/** The day a card belongs to: its newest work. */
export function dayOf(card: Pick<CanvasCard, 'entry' | 'tray'>): string | null {
  return card.entry?.date ?? card.tray?.date ?? null;
}

/** "Tidy up": the cards from before `before` (an ISO day) that can be put away, leaving
 *  pinned ones (`pinned` holds `kind:id` keys). */
export function tidyCandidates<C extends Pick<CanvasCard, 'kind' | 'id' | 'entry' | 'tray'>>(
  cards: C[],
  before: string,
  pinned: Set<string>,
): C[] {
  return cards.filter((c) => {
    const day = dayOf(c);
    return canPutAway(c) && !pinned.has(`${c.kind}:${c.id}`) && day !== null && day < before;
  });
}
