// Boards (rounds 8 and 12): the full screen you step into from the Desk. One is open at a
// time, keyed like `shiftbook_core::boards::Board` ("desk", "project:<id>", "daily", …),
// shown as its Notebook (its notes), its Board (its own canvas) or a List.

export type BoardTab = 'notebook' | 'board' | 'list';

export const boards = $state<{ key: string | null; tab: BoardTab; serial: number }>({
  key: null,
  tab: 'list',
  serial: 0,
});

/** Boards that only list other boards have no notebook or canvas. */
export function onlyLists(key: string): boolean {
  return key === 'desk' || key === 'projects' || key === 'meetings' || key === 'archive';
}

/** Step into a board (from the Desk, a box, a drawer or the tree). */
export function openBoard(key: string, tab?: BoardTab): void {
  boards.key = key;
  boards.tab = onlyLists(key) ? 'list' : (tab ?? (boards.tab === 'list' ? 'notebook' : boards.tab));
  boards.serial += 1;
}

export function closeBoard(): void {
  boards.key = null;
}
