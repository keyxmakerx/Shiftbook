// The tickets of one Desk card for some work (round 14): the entry's own, those on the same
// work's other days, and Tickets-tray tickets folded in. Each number shows once; it's done
// only when every place it appears is done, and ticking it ticks them all.
import type { CanvasCard, TrayItem } from './api';

/** Where a ticket number appears: an entry's ticket row, or a tray ticket. */
export type TicketSource =
  | { kind: 'entry'; id: string; done: boolean; note: string | null; date: string }
  | { kind: 'tray'; id: string; done: boolean; note: string | null; date: string; item: TrayItem };

export interface WorkTicket {
  number: string;
  done: boolean;
  /** The newest note written for it. */
  note: string | null;
  /** Newest first; the first takes a new note. */
  sources: TicketSource[];
}

/** The card's tickets, one per number, in the order they first appear (newest work first). */
export function workTickets(card: Pick<CanvasCard, 'entry' | 'work' | 'folded'>): WorkTicket[] {
  const sources: { number: string; source: TicketSource }[] = [];
  for (const e of [card.entry, ...(card.work ?? [])]) {
    if (!e) continue;
    for (const t of e.tickets) sources.push({ number: t.number, source: { kind: 'entry', id: t.id, done: t.done, note: t.note, date: e.date } });
  }
  for (const f of card.folded ?? []) {
    const t = f.item;
    sources.push({ number: t.ticket, source: { kind: 'tray', id: t.id, done: t.done, note: t.note, date: f.date, item: t } });
  }
  const out: WorkTicket[] = [];
  for (const { number, source } of sources) {
    const key = number.toUpperCase();
    let w = out.find((x) => x.number.toUpperCase() === key);
    if (!w) out.push((w = { number, done: true, note: null, sources: [] }));
    w.sources.push(source);
  }
  for (const w of out) {
    w.sources.sort((a, b) => b.date.localeCompare(a.date));
    w.done = w.sources.every((s) => s.done);
    w.note = w.sources.find((s) => s.note)?.note ?? null;
  }
  return out;
}
