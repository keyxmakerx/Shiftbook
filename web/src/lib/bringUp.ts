// Follow up (round 11 "Bring up…", one Follow up since round 15, card r15-things): any entry,
// ticket or note comes up in the bell and first in "Where was I?" on the day picked. A pin's
// chase day is the same thing, so a card has one follow-up day wherever it was set. Which days
// are offered, and where a day off moves to, are rules in crates/core/src/nextup.rs; the store
// keeps the offered days for today and each card's day (app.followUps).
import { api } from './api';
import type { CardKind } from './api';
import { refreshBell } from './bell.svelte';
import { askDay } from './dayPrompt.svelte';
import { addDays, dayMonth, daysBetween, longDay, weekdayLong, weekdayShort } from './format';
import type { MenuSection } from './menu.svelte';
import { refreshPins } from './pins.svelte';
import { app, attempt } from './store.svelte';
import { toast } from './toast.svelte';

async function changed(): Promise<void> {
  await Promise.all([refreshBell(), refreshPins()]);
}

/** Follow a card up on `on`, or on the next work day without it. */
export async function bringUp(kind: CardKind, id: string, title: string, on?: string): Promise<void> {
  const was = app.followUps[`${kind}:${id}`] ?? null;
  const item = await attempt(() => api.bringUp(app.today, kind, id, title || 'Untitled', on));
  if (!item) return;
  await changed();
  const when = item.from === app.today ? 'later today' : item.from ? longDay(item.from) : 'your next day';
  toast(`Follow up ${item.from === app.today ? when : `on ${when}`}`, 'Undo', () => void undo(kind, id, title, was));
}

async function undo(kind: CardKind, id: string, title: string, was: string | null): Promise<void> {
  const ok = await attempt(() =>
    (was ? api.bringUp(app.today, kind, id, title || 'Untitled', was < app.today ? app.today : was) : api.clearFollowUp(kind, id)).then(() => true),
  );
  if (ok) await changed();
}

/** "No follow-up": take the card's day away. */
export async function clearFollowUp(kind: CardKind, id: string, title: string): Promise<void> {
  const was = app.followUps[`${kind}:${id}`] ?? null;
  if (!(await attempt(() => api.clearFollowUp(kind, id).then(() => true)))) return;
  await changed();
  toast('No follow-up', 'Undo', () => void undo(kind, id, title, was));
}

/** A short name for an offered day: Tomorrow, a weekday within the week, or "Mon 2 November". */
export function dayChoiceLabel(day: string, today: string): string {
  const ahead = daysBetween(today, day);
  if (ahead === 0) return 'Later today';
  if (ahead === 1) return 'Tomorrow';
  if (ahead < 7) return weekdayLong(day);
  return `${weekdayShort(day)} ${dayMonth(day)}`;
}

/** "Follow up …": "later today", "tomorrow", "Thursday", or "since Thu 22 October" once past. */
export function followWords(day: string, today: string): string {
  if (day < today) return `since ${weekdayShort(day)} ${dayMonth(day)}`;
  const label = dayChoiceLabel(day, today);
  return daysBetween(today, day) <= 1 ? label.toLowerCase() : `on ${label}`;
}

/** Ask for any later day, then follow the card up then. */
async function pickDay(kind: CardKind, id: string, title: string): Promise<void> {
  const day = await askDay('Follow it up on…', addDays(app.today, 1));
  if (day) await bringUp(kind, id, title, day);
}

/**
 * The "Follow up" section of a card's menu (and a pin chip's): Later today, the next work day,
 * in 3 days, next week, Pick a day, and No follow-up when it has one. The card's day is ticked.
 */
export function bringUpSection(kind: CardKind, id: string, title: string): MenuSection {
  const current = app.followUps[`${kind}:${id}`] ?? null;
  const next = app.bringUpDays[0] ?? addDays(app.today, 1);
  const days: [string, string][] = [
    ['Later today', app.today],
    [dayChoiceLabel(next, app.today), next],
    ['In 3 days', addDays(app.today, 3)],
    ['Next week', addDays(app.today, 7)],
  ];
  const offered = new Set(days.map(([, d]) => d));
  const choices = days.map(([label, day]) => ({
    label,
    color: null,
    checked: current === day,
    act: () => void bringUp(kind, id, title, day),
  }));
  if (current && !offered.has(current)) {
    choices.push({ label: dayChoiceLabel(current, app.today), color: null, checked: true, act: () => void bringUp(kind, id, title, current) });
  }
  choices.push({ label: 'Pick a day…', color: null, checked: false, act: () => void pickDay(kind, id, title) });
  if (current) choices.push({ label: 'No follow-up', color: null, checked: false, act: () => void clearFollowUp(kind, id, title) });
  return { head: 'Follow up', choices };
}
