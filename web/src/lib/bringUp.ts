// "Bring up tomorrow" (round 11): any entry, ticket or note comes up first in "Where was I?"
// on the next work day. The rule for which day lives in crates/core/src/nextup.rs.
import { api } from './api';
import type { CardKind } from './api';
import { longDay } from './format';
import { app, attempt } from './store.svelte';
import { toast } from './toast.svelte';

export async function bringUp(kind: CardKind, id: string, title: string): Promise<void> {
  const item = await attempt(() => api.bringUp(app.today, kind, id, title || 'Untitled'));
  if (!item) return;
  toast(`Comes up first on ${item.from ? longDay(item.from) : 'your next day'}`, 'Undo', () => {
    void attempt(() => api.tickNextUp(item.id, true));
  });
}
