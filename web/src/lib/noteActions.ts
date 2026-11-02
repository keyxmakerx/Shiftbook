// Log a jot as an entry (round 14): its title says what was done, the rest becomes the
// entry's notes, ticket numbers in it become its tickets (rules in crates/core/src/notes.rs,
// jot_to_entry). It lands on today, or the day open on Day, and the jot moves to the archive.
import { api } from './api';
import type { Note } from './api';
import { fmtMinutes } from './format';
import type { MenuSection } from './menu.svelte';
import { app, attempt, flashEntry, refresh, timeButtons } from './store.svelte';
import { toast } from './toast.svelte';

/** Log `note` as an entry with `minutes`; `after` runs once it's done (and after Undo).
 * The entry takes the jot's spot on the Desk: storage copies a spot the owner gave it, and
 * `spot` is where a Desk card placed automatically was showing. */
export async function logNote(
  note: Note,
  minutes: number,
  after?: () => void,
  spot?: { x: number; y: number },
): Promise<void> {
  const day = app.view === 'day' ? app.selected : app.today;
  const entry = await attempt(() => api.logNote(note.id, day, minutes));
  if (!entry) return;
  if (spot) await attempt(() => api.canvasMove('entry', entry.id, spot.x, spot.y));
  await refresh();
  flashEntry(entry.id);
  after?.();
  toast(`Logged ${fmtMinutes(minutes)} of “${entry.description}”. The jot is archived.`, 'Undo', () => {
    void attempt(async () => {
      await api.deleteEntry(entry.id);
      await api.setNoteArchived(note.id, false);
    })
      .then(refresh)
      .then(() => after?.());
  });
}

/** The "Log it as an entry" choices for a jot's menu. */
export function logNoteSection(note: Note, after?: () => void, spot?: { x: number; y: number }): MenuSection {
  return {
    head: 'Log it as an entry',
    choices: timeButtons().map((m) => ({
      label: fmtMinutes(m),
      color: null,
      checked: false,
      act: () => void logNote(note, m, after, spot),
    })),
  };
}
