// The entries one Desk card stands for (round 14 folds the same work on several days into one
// card): the entry it shows plus the older ones in its `work`. A new project or name has to go
// to all of them. Changing only the shown one splits the card in two, and the older entry then
// looks like a copy that came from nowhere.
import type { Entry } from './api';

/** The shown entry first, then the folded ones. Tray roll-ups keep their own home and are left out. */
export function sameWork<E extends Pick<Entry, 'id' | 'isTray'>>(entry: E, also: readonly E[] = []): E[] {
  return [entry, ...also.filter((e) => e.id !== entry.id && !e.isTray)];
}
