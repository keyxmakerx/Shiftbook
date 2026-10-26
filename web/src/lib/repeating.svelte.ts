// Repeating things (round 15): bills, bins, renewals, certificates, patching. The rules (next
// due, warn days, the states and the words) come from crates/core/src/recurring.rs; this keeps
// the latest list and the one action used from the Desk, the bell and the Repeating view.
import { api } from './api';
import type { Recurring } from './api';
import type { MenuChoice } from './menu.svelte';
import { refreshBell } from './bell.svelte';
import { app, attempt, focusLog, refresh, setView } from './store.svelte';
import { startLine } from './renewal';
import { toast } from './toast.svelte';

export const repeating = $state({
  items: [] as Recurring[],
  /** A thing for the Repeating view to open (from the bell or the Desk). */
  open: { id: null as string | null, renew: false, serial: 0 },
});

/** Menu choices for filing work under a repeating thing (round 15). */
export function repeatingChoices(checked: string | null, act: (id: string) => void): MenuChoice[] {
  if (repeating.items.length === 0) void refreshRepeating();
  return repeating.items
    .filter((r) => r.state !== 'inactive' || r.id === checked)
    .map((r) => ({ label: `⟳ ${r.name}`, color: null, checked: r.id === checked, act: () => act(r.id) }));
}

export async function refreshRepeating(): Promise<void> {
  const items = await attempt(() => api.recurring(app.today));
  if (items) repeating.items = items;
}

/** The things that need doing: overdue, due soon, or an expiry date to move. */
export function needsAttention(items: Recurring[]): Recurring[] {
  return items.filter((r) => r.state === 'overdue' || r.state === 'due_soon' || r.state === 'renewed');
}

async function afterChange(): Promise<void> {
  await Promise.all([refreshRepeating(), refresh(), refreshBell()]);
}

/** Mark done today (a tick on the day), with Undo. */
export async function markDone(id: string, name: string): Promise<void> {
  const marked = await attempt(() => api.recurringDone(id, app.today));
  if (!marked) return;
  await afterChange();
  const now = repeating.items.find((r) => r.id === id);
  const next = now?.dueWords ? ` Next: ${now.dueWords}.` : '';
  toast(`${name}: done.${next}`, 'Undo', () => {
    void attempt(() => api.recurringUndo(marked).then(() => true)).then((ok) => {
      if (ok) void afterChange();
    });
  });
}

/** Renewed: it now runs to `expires`. */
export async function markRenewed(id: string, name: string, expires: string): Promise<boolean> {
  const marked = await attempt(() => api.recurringRenewed(id, expires, app.today));
  if (!marked) return false;
  await afterChange();
  toast(`${name}: renewed.`);
  return true;
}

/** Start renewal: open Log work with a line already filed under the thing. */
export function startWork(r: Recurring): void {
  focusLog(startLine(r.name, r.expires));
}

/** Go to the Repeating view with one thing open (and its Renewed box, for things that expire). */
export function openRepeating(id: string, renew = false): void {
  repeating.open = { id, renew, serial: repeating.open.serial + 1 };
  setView('repeating');
}
