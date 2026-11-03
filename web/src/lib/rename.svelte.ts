// Renaming an entry where it sits (round 15): a ✎ on its row or Desk card, F2, or Rename in its
// menu turns the name into a box. One at a time across Day and the Desk.
import type { Entry } from './api';
import { renameAll } from './entryActions';
import { app } from './store.svelte';

export const renaming = $state<{ id: string | null; shown: boolean }>({ id: null, shown: false });

/** Rename in place; when the entry isn't on screen as a row or card (in a pile, say), open its
 *  details, whose Name box is focused. */
export function startRename(id: string): void {
  renaming.id = id;
  renaming.shown = false;
  setTimeout(() => {
    if (renaming.id !== id || renaming.shown) return;
    renaming.id = null;
    app.drawer = id;
  }, 50);
}

export function stopRename(): void {
  renaming.id = null;
}

/** Save a new name, with Undo. A blank or unchanged name changes nothing. `also` is the same
 *  work on other days that a Desk card folds in: it gets the name too, so the card stays one. */
export async function renameEntry(entry: Entry, name: string, also: readonly Entry[] = []): Promise<boolean> {
  return renameAll(entry, name, also);
}
