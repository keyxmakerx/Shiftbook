// "Put on a ticket" for a note card on the Desk (one card per ticket, 18 Nov): the note's words
// go into the ticket's notepad (rules in crates/core/src/notes.rs, note_into_notepad) and the
// note moves to the archive, so a ticket and what was written about it are one card.
import type { CanvasCard, CanvasLink, CardKind } from './api';

/** Most tickets offered; the ones linked to the note come first. */
export const MAX_TICKET_CHOICES = 8;

const key = (c: { kind: CardKind; id: string }) => `${c.kind}:${c.id}`;

/** The ticket cards on the Desk a note card can go on: tickets linked to it by a line first
 *  (by ticket number or a shared word), then the open ones, then the rest, newest day first. */
export function ticketChoices(note: CanvasCard, cards: CanvasCard[], links: CanvasLink[]): CanvasCard[] {
  const me = key(note);
  const linked = new Set<string>();
  for (const l of links) {
    if (key(l.a) === me) linked.add(key(l.b));
    if (key(l.b) === me) linked.add(key(l.a));
  }
  const rank = (c: CanvasCard) => (linked.has(key(c)) ? 0 : c.tray && !c.tray.item.done ? 1 : 2);
  return cards
    .filter((c) => c.tray)
    .sort((a, b) => rank(a) - rank(b) || b.tray!.date.localeCompare(a.tray!.date))
    .slice(0, MAX_TICKET_CHOICES);
}

/** How many more lines a notepad has under its first one (blank lines don't count): a ticket
 *  card shows the first line and says how many more are inside. */
export function moreLines(note: string | null): number {
  if (!note) return 0;
  return Math.max(0, note.split('\n').filter((l) => l.trim()).length - 1);
}
