// The "few helpers" screen after setup (round 21): what its ticks save. Each is an ordinary
// setting, so everything here can be changed later in Settings.
import type { PrefKey, UseMode } from './api';

export interface HelperChoice {
  /** End-of-day wrap-up, and its time `HH:MM`. */
  wrapup: boolean;
  wrapupAt: string;
  /** The ▶ Start timer on entries. */
  timer: boolean;
  /** "I fill in a timesheet". */
  timesheet: boolean;
  /** The days worked, Monday first. */
  days: boolean[];
}

/** A full day, in minutes, for each day that is ticked. */
export const HELPER_DAY_MINUTES = 480;

/** What the screen starts with: wrap-up on at five, Monday to Friday. */
export function helperDefaults(): HelperChoice {
  return { wrapup: true, wrapupAt: '17:00', timer: false, timesheet: false, days: [true, true, true, true, true, false, false] };
}

/** True when Shiftbook is for home only: no Timesheet and no hours to fill, so those two
 *  helpers aren't offered. */
export function homeOnly(mode: UseMode): boolean {
  return mode === 'home';
}

/** The settings to save for what was ticked, in the order they are saved. */
export function helperPrefs(choice: HelperChoice, mode: UseMode): [PrefKey, string][] {
  const out: [PrefKey, string][] = [
    ['day.wrapup_at', choice.wrapup && /^\d\d:\d\d$/.test(choice.wrapupAt) ? choice.wrapupAt : ''],
    ['day.timer', choice.timer ? 'true' : 'false'],
  ];
  if (!homeOnly(mode)) {
    out.push(['timesheet.use', choice.timesheet ? 'true' : 'false']);
    out.push(['day.targets', choice.days.map((on) => (on ? HELPER_DAY_MINUTES : 0)).join(',')]);
  }
  return out;
}

/** "Mon–Fri", "Mon, Wed, Fri", "No days": the ticked days in a few words. */
export function daysSummary(days: boolean[]): string {
  const names = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const on = names.filter((_, i) => days[i]);
  if (on.length === 0) return 'No days';
  const first = days.indexOf(true);
  const last = days.lastIndexOf(true);
  const run = days.slice(first, last + 1).every(Boolean);
  return run && on.length > 2 ? `${names[first]}–${names[last]}` : on.join(', ');
}
