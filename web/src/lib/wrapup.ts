// The end-of-day wrap-up: when it shows, and the "Tomorrow I start with…" lines.

/** Most lines on the list (matches `shiftbook_core::nextup::MAX_NEXT_UP`). */
export const MAX_LINES = 7;

/** True from the wrap-up time (`HH:MM`, local) until midnight; never when it's off. */
export function isWrapupTime(now: Date, at: string | null): boolean {
  if (!at) return false;
  const [h, m] = at.split(':').map(Number);
  if (h === undefined || m === undefined || Number.isNaN(h) || Number.isNaN(m)) return false;
  return now.getHours() * 60 + now.getMinutes() >= h * 60 + m;
}

/** `17:30` as the user's clock shows it (5:30 pm, or 17:30). */
export function clockLabel(at: string): string {
  const [h = 0, m = 0] = at.split(':').map(Number);
  return new Date(2000, 0, 1, h, m).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

/** The lines with a blank one added after `index`, unless the list is full. */
export function insertLine(lines: string[], index: number): string[] {
  if (lines.length >= MAX_LINES) return lines;
  return [...lines.slice(0, index + 1), '', ...lines.slice(index + 1)];
}

/** The lines without `index`; the list always keeps one line to type in. */
export function removeLine(lines: string[], index: number): string[] {
  const next = lines.filter((_, i) => i !== index);
  return next.length ? next : [''];
}
