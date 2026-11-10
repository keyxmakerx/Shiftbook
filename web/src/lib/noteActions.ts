// Log a jot as an entry (round 14): its title says what was done, the rest becomes the
// entry's notes, ticket numbers in it become its tickets (rules in crates/core/src/notes.rs,
// jot_to_entry). It lands on today, or the day open on Day, and the jot moves to the archive.
import { api } from './api';
import type { Note, NoteKind } from './api';
import { fmtMinutes } from './format';
import type { MenuChoice, MenuItem, MenuSection } from './menu.svelte';
import { app, attempt, flashEntry, openWiki, refresh, timeButtons } from './store.svelte';
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

/** "Log it as an entry" as one row of a jot card's menu, its times opening in place (the
 *  daily-use review, 10 Nov). */
export function logNoteItem(note: Note, after?: () => void, spot?: { x: number; y: number }): MenuItem {
  return { label: 'Log it as an entry', icon: 'log', sub: [{ choices: logNoteSection(note, after, spot).choices }] };
}

const KINDS: { kind: NoteKind; label: string }[] = [
  { kind: 'jot', label: 'Jot' },
  { kind: 'daily', label: 'Daily' },
  { kind: 'meeting', label: 'Meeting' },
];

async function setKind(note: Note, kind: NoteKind): Promise<Note | undefined> {
  return attempt(() =>
    api.updateNote(note.id, { kind, title: note.title, body: note.body, projectId: note.projectId, tags: note.tags.join(', ') }),
  );
}

/** Change a note's kind, with Undo. */
export async function changeKind(note: Note, kind: NoteKind, after?: () => void): Promise<void> {
  if (note.kind === kind || !(await setKind(note, kind))) return;
  await refresh();
  after?.();
  const name = kind === 'page' ? 'wiki page' : kind === 'mop' ? 'MOP' : `${KINDS.find((k) => k.kind === kind)?.label ?? kind} note`;
  toast(`“${note.title}” is a ${name} now`, 'Undo', () => void setKind(note, note.kind).then(refresh).then(() => after?.()));
}

/** Make a note a wiki page and open it there (round 18), with Undo. */
export async function moveToWiki(note: Note, after?: () => void): Promise<void> {
  if (!(await setKind(note, 'page'))) return;
  await refresh();
  after?.();
  openWiki(note.id);
  toast(`“${note.title}” is a wiki page now. Use its ⋯ menu to put it in a space.`, 'Undo', () =>
    void setKind(note, note.kind).then(refresh).then(() => after?.()),
  );
}

/** Wiki pages and MOPs live in the wiki: they don't change kind from a menu. */
const inWiki = (note: Note) => note.kind === 'page' || note.kind === 'mop';

function kindChoices(note: Note, after?: () => void): MenuChoice[] {
  return KINDS.map((k) => ({
    label: k.label,
    color: null,
    checked: note.kind === k.kind,
    act: () => void changeKind(note, k.kind, after),
  }));
}

/** "Move to the wiki…" for a note's menu; null for wiki pages and MOPs. */
export function moveToWikiItem(note: Note, after?: () => void): MenuItem | null {
  return inWiki(note) ? null : { label: 'Move to the wiki…', icon: 'book', act: () => void moveToWiki(note, after) };
}

/** "Change to ▸ Jot / Daily / Meeting" and "Move to the wiki…" for any note's menu (round 18).
 *  Wiki pages and MOPs get neither: they live in the wiki. */
export function noteKindSections(note: Note, after?: () => void): MenuSection[] {
  const wiki = moveToWikiItem(note, after);
  if (!wiki) return [];
  return [{ head: 'Change to', choices: kindChoices(note, after) }, { items: [wiki] }];
}

/** "Kind" as one row of a note card's menu, showing the kind it is now; the kinds open in
 *  place (the daily-use review, 10 Nov). Null for wiki pages and MOPs. */
export function noteKindItem(note: Note, after?: () => void): MenuItem | null {
  if (inWiki(note)) return null;
  const now = KINDS.find((k) => k.kind === note.kind)?.label;
  return { label: 'Kind', icon: 'note', ...(now ? { value: now } : {}), sub: [{ choices: kindChoices(note, after) }] };
}
