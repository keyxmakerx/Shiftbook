// Which slide-out (round 12) is open: the one under an entry, a ticket or a Desk card. One at a
// time across Day and the Desk, keyed like Desk cards ("entry:<id>", "tray:<id>").

export const panel = $state<{ open: string | null; focusPad: boolean }>({ open: null, focusPad: false });

/** Open (or with a second click, fold away) the slide-out for `key`. */
export function togglePanel(key: string, focusPad = false): void {
  panel.focusPad = focusPad;
  panel.open = panel.open === key ? null : key;
}

/** Open the slide-out for `key` with the cursor in its notepad. */
export function openNotepad(key: string): void {
  panel.focusPad = true;
  panel.open = key;
}

export function closePanel(): void {
  panel.open = null;
}
