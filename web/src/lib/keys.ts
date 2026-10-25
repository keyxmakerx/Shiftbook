// Keyboard shortcuts, decided in one pure function so they can be tested.
//
// N focuses Log work, Ctrl+Space opens capture, 1–5 switch views, Ctrl+L locks, Ctrl +/−/0
// change the text size, Esc closes
// the top-most menu, capture or drawer. Plain letter and number keys are ignored while
// typing in a text box.

/** Desk is the Canvas (home); Day is the structured view for starting and ending the day. */
export type View = 'desk' | 'day' | 'timesheet' | 'notes' | 'settings';
export const VIEWS: readonly View[] = ['desk', 'day', 'timesheet', 'notes', 'settings'];
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
  | { kind: 'escape' }
  | { kind: 'zoom'; step: -1 | 0 | 1 }
  | { kind: 'menu-key' };

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

export function shortcutFor(e: KeyInfo): Shortcut | null {
  const mod = e.ctrlKey || e.metaKey;
  if (mod && !e.altKey && (e.code === 'Space' || e.key === ' ')) return { kind: 'capture' };
  if (mod && !e.altKey && e.key.toLowerCase() === 'l') return { kind: 'lock' };
  if (mod && !e.altKey && (e.key === '=' || e.key === '+')) return { kind: 'zoom', step: 1 };
  if (mod && !e.altKey && (e.key === '-' || e.key === '_')) return { kind: 'zoom', step: -1 };
  if (mod && !e.altKey && e.key === '0') return { kind: 'zoom', step: 0 };
  if (e.key === 'Escape') return { kind: 'escape' };
  if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) return { kind: 'menu-key' };
  if (mod || e.altKey || e.typing) return null;
  if (e.key === 'n' || e.key === 'N') return { kind: 'focus-log' };
  const n = Number(e.key);
  if (Number.isInteger(n) && n >= 1 && n <= VIEWS.length) {
    const view = VIEWS[n - 1];
    if (view) return { kind: 'view', view };
  }
  return null;
}

/** Every shortcut, as shown on the hold-Ctrl overlay. Keep in step with `shortcutFor`. */
export const SHORTCUT_LIST: readonly { keys: string; what: string }[] = [
  { keys: 'Ctrl Space', what: 'Log, note or find anything' },
  { keys: 'N', what: 'Jump to Log work' },
  { keys: 'Enter', what: 'In Log work: log it, or go to the time buttons' },
  { keys: '1  2  3  4  5', what: 'Desk, Day, Timesheet, Notes, Settings' },
  { keys: '↑  ↓', what: 'Move between entries' },
  { keys: 'Enter', what: 'Open the entry you are on' },
  { keys: 'Menu or Shift F10', what: 'Options for what you are on' },
  { keys: 'Ctrl +  Ctrl −', what: 'Bigger or smaller text (Ctrl 0 for normal)' },
  { keys: 'Ctrl L', what: 'Lock now' },
  { keys: 'Esc', what: 'Close the top menu or panel' },
];

/** How long Ctrl (or ⌘) must be held on its own before the overlay shows. */
export const HOLD_MS = 450;

/** True when this key is Ctrl or ⌘ pressed on its own, which starts the hold. */
export function startsHold(e: Pick<KeyInfo, 'key' | 'altKey' | 'shiftKey'>): boolean {
  return (e.key === 'Control' || e.key === 'Meta') && !e.altKey && !e.shiftKey;
}

/** Text sizes offered, in percent (see `shiftbook_core::prefs::TEXT_SIZES`). */
export const TEXT_SIZES = [90, 100, 115, 130] as const;

/** The text size one step bigger (1) or smaller (-1) than `now`, or normal (0). */
export function nextTextSize(now: number, step: -1 | 0 | 1): number {
  if (step === 0) return 100;
  const i = TEXT_SIZES.findIndex((t) => t >= now);
  const at = i < 0 ? TEXT_SIZES.length - 1 : i;
  return TEXT_SIZES[Math.min(TEXT_SIZES.length - 1, Math.max(0, at + step))]!;
}
