// "Jump back" (round 21): Ctrl+Tab shows what you had open lately and goes back to it. The
// list lives in memory and starts again at each unlock. Things count as opened when they
// show: a view, an entry's details or slide-out, a ticket's slide-out, a window, a note in
// Notes, a page in the Wiki.
import { tick, untrack } from 'svelte';
import { api } from './api';
import type { Note } from './api';
import { VIEW_NAMES, type View } from './keys';
import { openMopWindow } from './mops.svelte';
import { KIND_LABEL } from './notes';
import { panel } from './panel.svelte';
import { describe, forget, recentKey, step, visit, type Recent, type RecentKind } from './recent';
import { app, flashEntry, openNote, openWiki, selectDay, setView, shownViews } from './store.svelte';
import { openWindow, windows } from './windows.svelte';

export const recent = $state<{
  list: Recent[];
  /** The switcher is showing. */
  open: boolean;
  /** Opened from Find: it stays until something is picked (no key is being held). */
  sticky: boolean;
  sel: number;
  /** The note showing in Notes and the page showing in the Wiki. */
  noteShown: string | null;
  pageShown: string | null;
}>({ list: [], open: false, sticky: false, sel: 0, noteShown: null, pageShown: null });

// Bumped by every line added, so a view change that came with something else isn't listed.
let stamp = 0;
// A jump is under way: what it passes through isn't listed.
let quiet = false;

function add(kind: RecentKind, id: string, title: string, word: string, day?: string): void {
  stamp += 1;
  recent.list = visit(recent.list, { key: recentKey(kind, id), kind, id, title, word, day });
}

function noteWord(n: Note): string {
  if (n.kind === 'jot') return 'jot';
  if (n.kind === 'page') return 'page';
  if (n.kind === 'mop') return KIND_LABEL.mop;
  return `${KIND_LABEL[n.kind].toLowerCase()} note`;
}

/** A note is showing in Notes (or `null`: none is). */
export function sawNote(n: Note | null): void {
  recent.noteShown = n?.id ?? null;
  if (!n || quiet) return;
  // The note stands for Notes now: going back to it is going back there.
  recent.list = forget(recent.list, recentKey('view', 'notes'));
  add('note', n.id, n.title || 'Untitled', noteWord(n));
}

/** A page is showing in the Wiki. */
export function sawPage(n: Note): void {
  recent.pageShown = n.id;
  if (quiet) return;
  recent.list = forget(recent.list, recentKey('view', 'wiki'));
  add('page', n.id, n.title || 'Untitled', 'page');
}

async function sawEntry(id: string): Promise<void> {
  const known = app.entries.find((e) => e.id === id);
  add('entry', id, known?.description ?? '…', 'entry', known?.date);
  if (known) return;
  const key = recentKey('entry', id);
  try {
    const e = await api.getEntry(id);
    recent.list = describe(recent.list, key, { title: e.description, day: e.date });
  } catch {
    recent.list = forget(recent.list, key);
  }
}

async function sawTicket(id: string): Promise<void> {
  const key = recentKey('ticket', id);
  const had = recent.list.find((r) => r.key === key);
  add('ticket', id, had?.title ?? '…', 'ticket');
  if (had) return;
  const t = await api.trayItem(id).catch(() => null);
  recent.list = t ? describe(recent.list, key, { title: t.ticket }) : forget(recent.list, key);
}

async function sawNoteWindow(id: string): Promise<void> {
  const key = recentKey('note', id);
  const had = recent.list.find((r) => r.key === key);
  add('note', id, had?.title ?? '…', had?.word ?? 'note');
  if (had) return;
  try {
    const n = await api.getNote(id);
    recent.list = describe(recent.list, key, { title: n.title || 'Untitled', word: noteWord(n) });
  } catch {
    recent.list = forget(recent.list, key);
  }
}

/** A card showing in a slide-out or a window, keyed `kind:id` like Desk cards. */
function sawCard(key: string): void {
  const at = key.indexOf(':');
  const [kind, id] = [key.slice(0, at), key.slice(at + 1)];
  if (!id) return;
  if (kind === 'entry') void sawEntry(id);
  else if (kind === 'tray') void sawTicket(id);
  else if (kind === 'note') void sawNoteWindow(id);
}

/** Start watching what gets opened. Called once by the shell, so it starts fresh at unlock. */
export function watchRecent(): void {
  recent.list = [];
  recent.open = false;
  recent.noteShown = null;
  recent.pageShown = null;

  // A view counts when you go to it for itself, not on the way to something in it.
  $effect(() => {
    const view = app.view;
    untrack(() => {
      if (quiet) return;
      const before = stamp;
      setTimeout(() => {
        if (stamp === before && app.view === view && !quiet) add('view', view, VIEW_NAMES[view], 'view');
      }, 0);
    });
  });
  $effect(() => {
    const id = app.drawer;
    if (id && !untrack(() => quiet)) untrack(() => void sawEntry(id));
  });
  $effect(() => {
    const key = panel.open;
    if (key && !untrack(() => quiet)) untrack(() => sawCard(key));
  });
  $effect(() => {
    const key = windows.front;
    if (key && windows.loaded && !untrack(() => quiet)) untrack(() => sawCard(key));
  });
}

const cardKey = (r: Recent) => `${r.kind === 'ticket' ? 'tray' : r.kind}:${r.id}`;

/** True for the thing in front of you now: there is nowhere to jump to. */
function isCurrent(r: Recent): boolean {
  if (r.kind === 'view') return app.view === r.id;
  if (r.kind === 'page') return app.view === 'wiki' && recent.pageShown === r.id;
  if (windows.front === cardKey(r)) return true;
  // A slide-out only shows on the Desk and on Day.
  if (panel.open === cardKey(r) && (app.view === 'desk' || app.view === 'day')) return true;
  if (r.kind === 'entry') return app.drawer === r.id;
  if (r.kind === 'note') return app.view === 'notes' && recent.noteShown === r.id;
  return false;
}

/** What the switcher lists: everything lately open except what is in front now. */
export function jumpList(): Recent[] {
  return recent.list.filter((r) => !isCurrent(r) && (r.kind !== 'view' || shownViews().includes(r.id as View)));
}

/** Show the switcher. With `hold`, letting go of Ctrl picks; without, it waits for a pick.
 * False when there is nothing to jump back to. */
export function openRecent(hold: boolean, back = false): boolean {
  const n = jumpList().length;
  if (n === 0) return false;
  recent.sel = back ? n - 1 : 0;
  recent.sticky = !hold;
  recent.open = true;
  return true;
}

export function moveRecent(by: 1 | -1): void {
  recent.sel = step(recent.sel, by, jumpList().length);
}

export function closeRecent(): void {
  recent.open = false;
}

/** Go back to a thing, the way Find would open it. */
export async function jump(r: Recent): Promise<void> {
  recent.open = false;
  // Whatever had the keyboard stays behind.
  (document.activeElement as HTMLElement | null)?.blur?.();
  quiet = true;
  try {
    if (r.kind === 'view') {
      setView(r.id as View);
    } else if (r.kind === 'page') {
      openWiki(r.id);
    } else if (windows.list.some((w) => `${w.kind}:${w.id}` === cardKey(r))) {
      // It still has a window: bring that back.
      openWindow(r.kind === 'ticket' ? 'tray' : (r.kind as 'entry' | 'note'), r.id);
    } else if (r.kind === 'ticket') {
      openWindow('tray', r.id);
    } else if (r.kind === 'note') {
      if (r.word === KIND_LABEL.mop) openMopWindow(r.id);
      else openNote(r.id);
    } else {
      // An entry comes back on its day with its slide-out open.
      setView('day');
      if (r.day) await selectDay(r.day);
      flashEntry(r.id);
      panel.focusPad = false;
      panel.from = 'row';
      panel.open = cardKey(r);
    }
    // Let what just opened settle before it is listed again, first.
    await tick();
    await new Promise((done) => setTimeout(done, 0));
  } finally {
    quiet = false;
  }
  stamp += 1;
  recent.list = visit(recent.list, r);
}
