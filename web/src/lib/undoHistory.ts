// Ctrl+Z (round 15): every toast with an Undo button is remembered, newest last, so Ctrl+Z
// undoes the latest and typing "undo" in Find lists them. Undoing an older one also undoes
// everything after it, newest first. Nothing is kept across a lock (the page reloads).

/** Most changes remembered. */
export const MAX_CHANGES = 20;

export interface Change {
  id: number;
  /** The toast's text, e.g. "Logged 30m of printer". */
  text: string;
  /** When it happened, in ms since the epoch. */
  at: number;
  undo: () => void;
  /** Does it again after an undo, where the change knows how. */
  redo: (() => void) | null;
}

/** The list with `change` added, keeping only the newest `MAX_CHANGES`. */
export function remember(list: readonly Change[], change: Change): Change[] {
  return [...list, change].slice(-MAX_CHANGES);
}

/** Split at `id`: what stays, and what to undo (newest first). Unknown ids undo nothing. */
export function undoFrom(list: readonly Change[], id: number): { kept: Change[]; undo: Change[] } {
  const i = list.findIndex((c) => c.id === id);
  if (i < 0) return { kept: [...list], undo: [] };
  return { kept: list.slice(0, i), undo: list.slice(i).reverse() };
}

/** "just now", "2 min ago", "1 h ago". */
export function agoText(at: number, now: number): string {
  const min = Math.floor((now - at) / 60_000);
  if (min < 1) return 'just now';
  if (min < 60) return `${min} min ago`;
  return `${Math.floor(min / 60)} h ago`;
}
