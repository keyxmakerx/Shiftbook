// Renaming an entry where it sits (round 15): a ✎ on its row or Desk card, F2, or Rename in its
// menu turns the name into a box. One at a time across Day and the Desk.
import { api } from './api';
import type { Entry } from './api';
import { input } from './entryActions';
import { app, attempt, putEntry, refresh } from './store.svelte';
import { toast } from './toast.svelte';

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

/** Save a new name, with Undo. A blank or unchanged name changes nothing. */
export async function renameEntry(entry: Entry, name: string): Promise<boolean> {
  const next = name.trim();
  if (!next || next === entry.description) return false;
  const before = entry.description;
  const updated = await attempt(() => api.updateEntry(entry.id, { ...input(entry, {}), description: next }));
  if (!updated) return false;
  putEntry(updated);
  void refresh();
  toast(`Renamed to “${next}”`, 'Undo', () => {
    void attempt(() => api.updateEntry(updated.id, { ...input(updated, {}), description: before })).then(refresh);
  });
  return true;
}
