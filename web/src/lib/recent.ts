// "Jump back" (round 21): the list of things you had open lately, newest first. This is only
// the list itself; what counts as opening something lives in recent.svelte.ts.

export type RecentKind = 'view' | 'entry' | 'ticket' | 'note' | 'page';

export interface Recent {
  /** `kind:id`, one line per thing. */
  key: string;
  kind: RecentKind;
  id: string;
  title: string;
  /** What it is, in a word or two: "meeting note", "ticket", "view". */
  word: string;
  /** An entry's day, to go back to it. */
  day?: string;
}

/** How many things the list remembers. */
export const MAX_RECENT = 8;

export const recentKey = (kind: RecentKind, id: string) => `${kind}:${id}`;

/** `item` was just opened: it goes first, and its older line (same key) goes. */
export function visit(list: readonly Recent[], item: Recent, max = MAX_RECENT): Recent[] {
  return [item, ...list.filter((r) => r.key !== item.key)].slice(0, max);
}

/** Fill in what a line is called once it is known. A note that turns out to be a wiki page
 * changes kind (and key) too. */
export function describe(list: readonly Recent[], key: string, facts: Partial<Omit<Recent, 'key' | 'id'>>): Recent[] {
  const out: Recent[] = [];
  for (const r of list) {
    if (r.key !== key) {
      out.push(r);
      continue;
    }
    const kind = facts.kind ?? r.kind;
    out.push({ ...r, ...facts, kind, key: recentKey(kind, r.id) });
  }
  // Renaming a key can leave two lines for one thing: the first (newest) stays.
  return out.filter((r, i) => out.findIndex((o) => o.key === r.key) === i);
}

/** Take a line out (the thing is gone). */
export function forget(list: readonly Recent[], key: string): Recent[] {
  return list.filter((r) => r.key !== key);
}

/** Move the highlight by one, round the ends. */
export function step(index: number, by: 1 | -1, length: number): number {
  if (length < 1) return 0;
  return (index + by + length) % length;
}
