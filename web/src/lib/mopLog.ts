// The run log of a MOP on its ticket (round 18; the day, the finish and "what I saw" lines
// since round 21): what gets pasted into the change record. The times are shown in local
// time, which is why this is built here; what is stored (ticks, start, finish, what was
// seen) and its rules live in crates/core/src/change.rs.
import type { CheckLine, MopRun } from './api';

/** Steps still to deal with: not ticked, and not crossed out under a ticked heading or step. */
export function stepsLeft(checks: CheckLine[]): number {
  return checks.filter((c) => c.key && !c.ticked && !c.crossed).length;
}

/** How times and days are written; the defaults follow the computer's own settings. */
export interface LogWords {
  /** A time of day, e.g. "22:04". */
  clock(ts: string): string;
  /** A day, e.g. "Wed 18 Nov 2026". */
  day(ts: string): string;
}

export const localWords: LogWords = {
  clock(ts) {
    const d = new Date(ts);
    return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  },
  day(ts) {
    const d = new Date(ts);
    return Number.isNaN(d.getTime())
      ? ''
      : d.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' });
  },
};

/** "1h 27m" between two times; '' when either is missing or they run backwards. */
export function took(from: string | null, to: string | null): string {
  if (!from || !to) return '';
  const minutes = Math.round((new Date(to).getTime() - new Date(from).getTime()) / 60000);
  if (!Number.isFinite(minutes) || minutes < 1) return '';
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h === 0 ? `${m}m` : m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/**
 * The run log as text. Line one names the ticket and the MOP; line two says where it stands,
 * the day, when it started and finished and how long it took. Then every line of the MOP with
 * the time it was ticked, and what was seen at a step indented under it.
 */
export function runLog(
  run: MopRun,
  title: string,
  status: string,
  lines: string[],
  checks: CheckLine[],
  words: LogWords = localWords,
): string {
  const at = (key: string) => run.tickTimes.find((t) => t.key === key)?.at ?? null;
  const first = run.startedAt ?? run.tickTimes.map((t) => t.at).sort()[0] ?? run.finishedAt;
  const head = [status];
  if (first) head.push(words.day(first));
  if (run.startedAt) head.push(`started ${words.clock(run.startedAt)}`);
  if (run.finishedAt) {
    // A run over midnight says which day it finished.
    const otherDay = first && words.day(first) !== words.day(run.finishedAt);
    head.push(`finished ${otherDay ? `${words.day(run.finishedAt)} ` : ''}${words.clock(run.finishedAt)}`);
    const long = took(run.startedAt, run.finishedAt);
    if (long) head.push(`took ${long}`);
  }
  const times = lines.map((_, i) => {
    const c = checks[i];
    return c?.ticked ? words.clock(at(c.key) ?? '') : '';
  });
  const pad = Math.max(5, ...times.map((t) => t.length));
  const out = [`${run.ticket} · ${title}`, head.join(' · '), ''];
  lines.forEach((line, i) => {
    if (!line.trim()) return;
    const c = checks[i];
    const mark = c?.ticked ? '✓' : c?.crossed ? '–' : '☐';
    out.push(`${times[i]!.padEnd(pad)} ${mark} ${line.trim()}`);
    const saw = c?.key ? run.seen.find((s) => s.key === c.key)?.text : undefined;
    if (saw) for (const l of saw.split('\n')) out.push(`${' '.repeat(pad)}     ${l}`.trimEnd());
  });
  return out.join('\n');
}
