// Which slide-out (round 12) is open: the one under an entry, a ticket or a Desk card. One at a
// time across Day and the Desk, keyed like Desk cards ("entry:<id>", "tray:<id>").
//
// On the Desk a ticket shows twice, as a card and as a row in the Tickets tray beside it, and
// both have the same key. `from` says which of the two was opened, so the same notepad is never
// open twice at once (each keeps its own draft, and the second would save over the first).

/** Where a slide-out was opened: under a Desk card, or under a row in a list. */
export type PanelFrom = 'card' | 'row';

export const panel = $state<{ open: string | null; focusPad: boolean; from: PanelFrom | null }>({
  open: null,
  focusPad: false,
  from: null,
});

/** True when the slide-out for `key` is open here (`at`): opened here, or from a menu that
 *  doesn't say where. */
export function panelOpenAt(key: string, at: PanelFrom): boolean {
  return panel.open === key && (panel.from === null || panel.from === at);
}

/** Open (or with a second click, fold away) the slide-out for `key`. */
export function togglePanel(key: string, focusPad = false, from: PanelFrom | null = null): void {
  const close = panel.open === key && (panel.from === null || from === null || panel.from === from);
  panel.focusPad = focusPad;
  panel.from = close ? null : from;
  panel.open = close ? null : key;
}

/** Open the slide-out for `key` with the cursor in its notepad. */
export function openNotepad(key: string, from: PanelFrom | null = null): void {
  panel.focusPad = true;
  panel.from = from;
  panel.open = key;
}

export function closePanel(): void {
  panel.open = null;
  panel.from = null;
}
