// "Same as yesterday": which entries from the last working day to offer as one-click
// buttons under Log work. Pure, so it can be tested.
import type { Entry } from './api';

/** At most this many buttons. */
export const MAX_REPEATS = 4;

/**
 * The latest day before `day` that has entries, from `earlier` (entries of the days
 * before, any order), minus the tray's roll-up and anything already logged on `day`
 * under the same title. Biggest first.
 */
export function repeatCandidates(day: string, earlier: Entry[], onDay: Entry[]): Entry[] {
  const before = earlier.filter((e) => e.date < day && !e.isTray && e.minutes > 0);
  if (before.length === 0) return [];
  const last = before.reduce((d, e) => (e.date > d ? e.date : d), before[0]!.date);
  const done = new Set(onDay.map((e) => e.description.trim().toLowerCase()));
  const seen = new Set<string>();
  return before
    .filter((e) => e.date === last)
    .sort((a, b) => b.minutes - a.minutes)
    .filter((e) => {
      const key = e.description.trim().toLowerCase();
      if (!key || done.has(key) || seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .slice(0, MAX_REPEATS);
}
