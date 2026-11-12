// Keyboard shortcuts, decided in one pure function so they can be tested.
//
// N focuses Log work, Ctrl+Space or / opens capture (Find), the number keys switch tabs and
// the next one up opens the Organise menu (round 18; one fewer at home, where the Timesheet
// is hidden), Ctrl+, opens Settings, Ctrl+L locks, Ctrl+Z undoes and Ctrl+Shift+Z redoes,
// Ctrl +/−/0 change the text size, Esc closes the top-most menu, capture or drawer. Plain letter and number keys are ignored while
// typing in a text box.

import { TEXT_SIZES } from './api/generated/constants';

/** Desk is the Canvas (home); Day is the structured view for starting and ending the day. */
export type View = 'desk' | 'day' | 'timesheet' | 'wiki' | 'notes' | 'repeating' | 'lists' | 'items' | 'projects' | 'settings';

/** Tabs are where you work (round 18; Notes became a tab, asked in chat 6 Nov 2026). */
export const TABS: readonly View[] = ['desk', 'day', 'timesheet', 'notes', 'wiki'];
/** The Organise menu: places you go to look after your things. */
export const ORGANISE: readonly View[] = ['items', 'projects', 'repeating', 'lists'];

/** The tabs shown: without the Timesheet at home (round 15). */
export function tabsFor(work: boolean): readonly View[] {
  return work ? TABS : TABS.filter((v) => v !== 'timesheet');
}

/** Every view there is in this mode: the tabs, the Organise menu, then Settings. */
export function viewsFor(work: boolean): readonly View[] {
  return [...tabsFor(work), ...ORGANISE, 'settings'];
}

export const VIEWS: readonly View[] = viewsFor(true);

/** Tab names. */
export const VIEW_NAMES: Record<View, string> = {
  desk: 'Desk',
  day: 'Day',
  timesheet: 'Timesheet',
  wiki: 'Wiki',
  notes: 'Notes',
  repeating: 'Repeating',
  lists: 'Lists',
  items: 'Tickets',
  projects: 'Projects',
  settings: 'Settings',
};
export type SettingsSection =
  | 'look'
  | 'day'
  | 'projects'
  | 'timesheet'
  | 'notes'
  | 'fields'
  | 'responses'
  | 'meetings'
  | 'backups'
  | 'sync'
  | 'security';

export type Shortcut =
  | { kind: 'capture' }
  | { kind: 'lock' }
  | { kind: 'focus-log' }
  | { kind: 'view'; view: View }
  | { kind: 'organise' }
  | { kind: 'escape' }
  | { kind: 'zoom'; step: -1 | 0 | 1 }
  | { kind: 'menu-key' }
  | { kind: 'just-today' }
  | { kind: 'undo' }
  | { kind: 'redo' };

export interface KeyInfo {
  key: string;
  code?: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  /** True when focus is in an input, textarea, select or editable element. */
  typing: boolean;
}

export function shortcutFor(e: KeyInfo, tabs: readonly View[] = TABS): Shortcut | null {
  const mod = e.ctrlKey || e.metaKey;
  if (mod && !e.altKey && e.key === ',') return { kind: 'view', view: 'settings' };
  if (mod && !e.altKey && (e.code === 'Space' || e.key === ' ')) return { kind: 'capture' };
  if (mod && !e.altKey && e.key.toLowerCase() === 'l') return { kind: 'lock' };
  if (mod && !e.altKey && (e.key === '=' || e.key === '+')) return { kind: 'zoom', step: 1 };
  if (mod && !e.altKey && (e.key === '-' || e.key === '_')) return { kind: 'zoom', step: -1 };
  if (mod && !e.altKey && e.key === '0') return { kind: 'zoom', step: 0 };
  // In a text box Ctrl+Z stays the box's own undo.
  if (mod && !e.altKey && !e.typing && e.key.toLowerCase() === 'z') return { kind: e.shiftKey ? 'redo' : 'undo' };
  if (mod && !e.altKey && !e.typing && e.key.toLowerCase() === 'y') return { kind: 'redo' };
  if (e.key === 'Escape') return { kind: 'escape' };
  if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) return { kind: 'menu-key' };
  if (mod || e.altKey || e.typing) return null;
  if (e.key === 'n' || e.key === 'N') return { kind: 'focus-log' };
  if (e.key === '/') return { kind: 'capture' };
  if (e.key === 't' || e.key === 'T') return { kind: 'just-today' };
  const n = Number(e.key);
  if (Number.isInteger(n) && n >= 1 && n <= tabs.length) {
    const view = tabs[n - 1];
    if (view) return { kind: 'view', view };
  }
  if (n === tabs.length + 1) return { kind: 'organise' };
  return null;
}

/** Every shortcut, as shown on the hold-Ctrl overlay. Keep in step with `shortcutFor`. */
export function shortcutList(tabs: readonly View[] = TABS): readonly { keys: string; what: string }[] {
  return [
    ...SHORTCUT_LIST.slice(0, 3),
    { keys: tabs.map((_, i) => String(i + 1)).join('  '), what: tabs.map((v) => VIEW_NAMES[v]).join(', ') },
    { keys: String(tabs.length + 1), what: `Organise: ${ORGANISE.map((v) => VIEW_NAMES[v]).join(', ')}` },
    { keys: 'Ctrl ,', what: 'Settings' },
    ...SHORTCUT_LIST.slice(4),
  ];
}

export const SHORTCUT_LIST: readonly { keys: string; what: string }[] = [
  { keys: 'Ctrl Space  or  /', what: 'Log, note or find anything (type "help" for everything)' },
  { keys: 'N', what: 'Jump to Log work' },
  { keys: 'Enter', what: 'In Log work: log it, or go to the time buttons' },
  { keys: '1  2  3  4', what: 'Desk, Day, Timesheet, Wiki' },
  { keys: '↑  ↓', what: 'Move between entries' },
  { keys: 'Enter', what: 'Open the entry you are on' },
  { keys: 'Menu or Shift F10', what: 'Options for what you are on' },
  { keys: 'Ctrl +  Ctrl −', what: 'Bigger or smaller text (Ctrl 0 for normal)' },
  { keys: 'T', what: 'On the Desk: just today (other days fade)' },
  { keys: 'Ctrl Z', what: 'Undo the last change (type "undo" in Find for the list)' },
  { keys: 'Ctrl Shift Z', what: 'Redo it' },
  { keys: 'Ctrl L', what: 'Lock now' },
  { keys: 'Esc', what: 'Close the top menu or panel' },
];

/** How long Ctrl (or ⌘) must be held on its own before the overlay shows. */
export const HOLD_MS = 450;

/** True when this key is Ctrl or ⌘ pressed on its own, which starts the hold. */
export function startsHold(e: Pick<KeyInfo, 'key' | 'altKey' | 'shiftKey'>): boolean {
  return (e.key === 'Control' || e.key === 'Meta') && !e.altKey && !e.shiftKey;
}

/** Text sizes offered, in percent (core's `TEXT_SIZES`). */
export { TEXT_SIZES };

/** The text size one step bigger (1) or smaller (-1) than `now`, or normal (0). */
export function nextTextSize(now: number, step: -1 | 0 | 1): number {
  if (step === 0) return 100;
  const i = TEXT_SIZES.findIndex((t) => t >= now);
  const at = i < 0 ? TEXT_SIZES.length - 1 : i;
  return TEXT_SIZES[Math.min(TEXT_SIZES.length - 1, Math.max(0, at + step))]!;
}
