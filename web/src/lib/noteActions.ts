// Log a jot as an entry (round 14): its title says what was done, the rest becomes the
// entry's notes, ticket numbers in it become its tickets (rules in crates/core/src/notes.rs,
// jot_to_entry). It lands on today, or the day open on Day, and the jot moves to the archive.
import { api } from './api';
import type { Note } from './api';
import { fmtMinutes } from './format';
import type { MenuSection } from './menu.svelte';
import { app, attempt, flashEntry, refresh } from './store.svelte';
import { toast } from './toast.svelte';

const TIMES = [15, 30, 45, 60, 120];

/** Log `note` as an entry with `minutes`; `after` runs once it's done (and after Undo). */
export async function logNote(note: Note, minutes: number, after?: () => void): Promise<void> {
  const day = app.view === 'day' ? app.selected : app.today;
  const entry = await attempt(() => api.logNote(note.id, day, minutes));
  if (!entry) return;
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
export function logNoteSection(note: Note, after?: () => void): MenuSection {
  return {
    head: 'Log it as an entry',
    choices: TIMES.map((m) => ({ label: fmtMinutes(m), color: null, checked: false, act: () => void logNote(note, m, after) })),
  };
}
