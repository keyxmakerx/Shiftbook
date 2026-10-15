// Keyboard shortcuts, decided in one pure function so they can be tested.
//
// N focuses Log work, Ctrl+Space opens capture, 1–3 switch views, Ctrl+L locks, Esc closes
// the top-most menu, capture or drawer. Plain letter and number keys are ignored while
// typing in a text box.

export type View = 'desk' | 'timesheet' | 'customize';
export const VIEWS: readonly View[] = ['desk', 'timesheet', 'customize'];

export type Shortcut =
  | { kind: 'capture' }
  | { kind: 'lock' }
  | { kind: 'focus-log' }
  | { kind: 'view'; view: View }
  | { kind: 'escape' }
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
