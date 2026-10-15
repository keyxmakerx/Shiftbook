// Display helpers for dates. Display only: business rules about dates (week-of, due dates)
// live in crates/core.

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

/** Local calendar date as ISO `YYYY-MM-DD` (not UTC, so late-evening entries stay on the right day). */
export function toIsoDate(date: Date): string {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

/** Friendly long date for headings, e.g. "Thursday, 15 October 2026" (wording follows the locale). */
export function formatLongDate(date: Date, locale?: string): string {
  return new Intl.DateTimeFormat(locale, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(date);
}
