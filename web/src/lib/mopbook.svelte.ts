// The MOP notebook window (round 16, card r15-mopbook): one floating window with a tab per
// open MOP, like Notepad++. It rolls up, minimises to the bottom bar or fills the screen,
// and shows a MOP Numbered (line numbers, colours, counts), Minimal (just the text) or as a
// Checklist. Where it is and what's open is one preference (`ui.mopbook`, kept in bounds by
// crates/core/src/mop.rs) so it comes back as it was.
import { api } from './api';
import type { MopBook, MopFile, MopView, WindowMode } from './api';
import { formatBook, readFiles } from './mop';
import { phone } from './phone.svelte';
import { app } from './store.svelte';

const SAVE_AFTER_MS = 600;
const MAX_TABS = 20;

export const book = $state<MopBook & { loaded: boolean }>({
  loaded: false,
  shown: false,
  mode: 'open',
  x: 120,
  y: 60,
  w: 640,
  h: 460,
  view: 'numbered',
  numbers: true,
  tabs: [],
  active: null,
});

/** Each open tab's title once loaded, and which have unsaved typing. */
export const tabInfo = $state<{ titles: Record<string, string>; dirty: Record<string, boolean> }>({ titles: {}, dirty: {} });

let saving: ReturnType<typeof setTimeout> | null = null;

function save() {
  if (saving) clearTimeout(saving);
  saving = setTimeout(() => {
    saving = null;
    void api.savePref('ui.mopbook', formatBook(book)).catch(() => undefined);
  }, SAVE_AFTER_MS);
}

/** Take the notebook as it was last time (after unlock). */
export function loadMopBook(): void {
  const saved = app.prefs?.mopbook;
  if (saved) Object.assign(book, saved, { tabs: [...saved.tabs] });
  book.loaded = true;
}

/** Show the notebook, with `id` open in front when given. */
export function openMopBook(id?: string | null): void {
  if (id) {
    if (!book.tabs.includes(id)) {
      book.tabs.push(id);
      while (book.tabs.length > MAX_TABS) book.tabs.shift();
    }
    book.active = id;
  }
  book.shown = true;
  if (phone.on) book.mode = 'full';
  else if (book.mode === 'min' || book.mode === 'rolled') book.mode = 'open';
  save();
}

export function closeMopBook(): void {
  book.shown = false;
  save();
}

export function setBookMode(mode: WindowMode): void {
  book.mode = mode;
  save();
}

export function setBookView(view: MopView): void {
  book.view = view;
  save();
}

export function toggleNumbers(): void {
  book.numbers = !book.numbers;
  save();
}

export function moveBook(x: number, y: number): void {
  book.x = x;
  book.y = Math.max(0, y);
  save();
}

export function resizeBook(w: number, h: number): void {
  book.w = Math.max(360, w);
  book.h = Math.max(220, h);
  save();
}

export function showTab(id: string): void {
  book.active = id;
  save();
}

/** Close one tab; the one beside it comes to the front. */
export function closeTab(id: string): void {
  const i = book.tabs.indexOf(id);
  if (i < 0) return;
  book.tabs.splice(i, 1);
  if (book.active === id) book.active = book.tabs[Math.min(i, book.tabs.length - 1)] ?? null;
  delete tabInfo.titles[id];
  delete tabInfo.dirty[id];
  save();
}

/** The bottom bar button: show it, or tuck it away. */
export function toggleBookFromBar(): void {
  if (!book.shown || book.mode === 'min') openMopBook();
  else setBookMode('min');
}

/** Files dropped or picked, waiting in the "Copy MOP files in" list. */
export const mopImport = $state<{ files: MopFile[] | null }>({ files: null });

/** Read files and show them in the "Copy MOP files in" list. */
export async function offerFiles(files: Iterable<File>): Promise<void> {
  const read = await readFiles(files);
  if (read.length) mopImport.files = read;
}

/** Ask for MOP files with the system's file picker (the "Add MOP files…" button). */
export function pickMopFiles(): void {
  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.accept = '.txt,.text,.md,.log,.mop,text/plain';
  input.onchange = () => {
    if (input.files?.length) void offerFiles(input.files);
  };
  input.click();
}
