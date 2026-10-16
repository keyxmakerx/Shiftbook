// Display helpers only. Rules (week-of, totals, hours parsing) live in crates/core; these
// just turn values the app already computed into text.

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

/** A `YYYY-MM-DD` string as a local Date at midnight. */
export function fromIso(iso: string): Date {
  const [y, m, d] = iso.split('-').map(Number);
  return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1);
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

/** A local Date as `YYYY-MM-DD`. */
export function toIso(date: Date): string {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

/** `iso` moved by `days` calendar days. */
export function addDays(iso: string, days: number): string {
  const d = fromIso(iso);
  d.setDate(d.getDate() + days);
  return toIso(d);
}

/** Whole days from `a` to `b`. */
export function daysBetween(a: string, b: string): number {
  return Math.round((fromIso(b).getTime() - fromIso(a).getTime()) / 86_400_000);
}

export function weekdayLong(iso: string): string {
  return WEEKDAYS[fromIso(iso).getDay()] ?? '';
}

/** Saturday or Sunday. */
export function isWeekend(iso: string): boolean {
  const day = fromIso(iso).getDay();
  return day === 0 || day === 6;
}

export function weekdayShort(iso: string): string {
  return weekdayLong(iso).slice(0, 3);
}

/** "Thursday 15 October". */
export function longDay(iso: string): string {
  const d = fromIso(iso);
  return `${WEEKDAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

/** "15 October". */
export function dayMonth(iso: string): string {
  const d = fromIso(iso);
  return `${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

/** Day of the month as a number. */
export function dayOfMonth(iso: string): number {
  return fromIso(iso).getDate();
}

/** "Today", "Yesterday" or "Looking back" / "Ahead". */
export function relativeDay(iso: string, today: string): string {
  const diff = daysBetween(today, iso);
  if (diff === 0) return 'Today';
  if (diff === -1) return 'Yesterday';
  return diff < 0 ? 'Looking back' : 'Ahead';
}

/**
 * Minutes as decimal hours with at most two decimals and no trailing zeros, like core's
 * `format_hours` (90 → "1.5", 75 → "1.25", 20 → "0.33"). Used only while numbers count up.
 */
export function fmtHours(minutes: number): string {
  const hundredths = Math.floor((Math.max(0, minutes) * 100 + 30) / 60);
  const whole = Math.floor(hundredths / 100);
  const rest = hundredths % 100;
  if (rest === 0) return String(whole);
  if (rest % 10 === 0) return `${whole}.${rest / 10}`;
  return `${whole}.${pad2(rest)}`;
}

/** Minutes as "15m", "1h", "1h 30m". */
export function fmtMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m}m`;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** A project's colour, or the neutral one. */
export function colorOf(color: string | null | undefined): string {
  return color ?? 'var(--none)';
}
