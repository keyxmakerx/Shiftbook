// Several lines pasted into Log work, one entry each (round 21). Which lines count and what
// each means comes from the app's core (`capture_lines`); this is the preview's sums and words.

import type { CaptureLine } from './api/DataApi';

/** One line in the preview: what to log and how long (null until a time is picked). */
export interface ManyRow {
  text: string;
  tickets: string[];
  projectId: string | null;
  minutes: number | null;
}

/** The preview's rows for the lines read. A line with a time in it starts with that time. */
export function manyRows(lines: CaptureLine[]): ManyRow[] {
  return lines.map((l) => ({
    text: l.text,
    tickets: l.capture.tickets,
    projectId: l.capture.projectId,
    minutes: l.capture.minutes,
  }));
}

/** Text that would be several lines in Log work (so a paste of it gets the preview). */
export function isManyLines(pasted: string): boolean {
  return pasted.split('\n').filter((l) => l.trim() !== '').length > 1;
}

/** How many rows still need a time. */
export function missingTimes(rows: ManyRow[]): number {
  return rows.filter((r) => r.minutes === null).length;
}

/** The minutes all the rows add up to (rows with no time yet count as nothing). */
export function totalMinutes(rows: ManyRow[]): number {
  return rows.reduce((sum, r) => sum + (r.minutes ?? 0), 0);
}

/** Give every row that has no time yet `minutes`. */
export function fillTimes(rows: ManyRow[], minutes: number): ManyRow[] {
  return rows.map((r) => (r.minutes === null ? { ...r, minutes } : r));
}

/** The pasted text as the one line the box used to make of it. */
export function oneLine(pasted: string): string {
  return pasted.split(/\s+/).filter(Boolean).join(' ');
}
