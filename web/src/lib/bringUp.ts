// "Bring up…" (round 11, more days in round 15): any entry, ticket or note comes up first in
// "Where was I?" on the day picked. Which days are offered, and where a day off moves to,
// are rules in crates/core/src/nextup.rs; the store keeps the offered days for today.
import { api } from './api';
import type { CardKind } from './api';
import { askDay } from './dayPrompt.svelte';
import { addDays, dayMonth, daysBetween, longDay, weekdayLong, weekdayShort } from './format';
import type { MenuSection } from './menu.svelte';
import { app, attempt } from './store.svelte';
import { toast } from './toast.svelte';

/** Bring a card up on `on`, or on the next work day without it. */
export async function bringUp(kind: CardKind, id: string, title: string, on?: string): Promise<void> {
  const item = await attempt(() => api.bringUp(app.today, kind, id, title || 'Untitled', on));
  if (!item) return;
  toast(`Comes up first on ${item.from ? longDay(item.from) : 'your next day'}`, 'Undo', () => {
    void attempt(() => api.tickNextUp(item.id, true));
  });
}

/** A short name for an offered day: Tomorrow, a weekday within the week, or "Mon 2 November". */
export function dayChoiceLabel(day: string, today: string): string {
  const ahead = daysBetween(today, day);
  if (ahead === 1) return 'Tomorrow';
  if (ahead < 7) return weekdayLong(day);
  return `${weekdayShort(day)} ${dayMonth(day)}`;
}

/** Ask for any later day, then bring the card up then. */
async function pickDay(kind: CardKind, id: string, title: string): Promise<void> {
  const day = await askDay('Bring it up on…', addDays(app.today, 1));
  if (day) await bringUp(kind, id, title, day);
}

/**
 * The "Bring up on…" section of a card's menu: the offered days after the first (that one
 * is the menu's "Bring up tomorrow"), then Pick a day.
 */
export function bringUpSection(kind: CardKind, id: string, title: string): MenuSection {
  const later = app.bringUpDays.slice(1);
  return {
    head: 'Bring up on…',
    choices: [
      ...later.map((day) => ({
        label: dayChoiceLabel(day, app.today),
        color: null,
        checked: false,
        act: () => void bringUp(kind, id, title, day),
      })),
      { label: 'Pick a day…', color: null, checked: false, act: () => void pickDay(kind, id, title) },
    ],
  };
}
