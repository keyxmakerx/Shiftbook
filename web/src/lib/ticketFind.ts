// Finding a ticket again from somewhere else (asked in chat, 6 Nov 2026): a MOP's "For
// CHG…", a wiki page's On tickets, a MOP in Notes. "Go to the ticket" opens the Tickets page
// showing just that ticket (its status, time, project and MOP), on the Open or Done tab it
// belongs to. A ticket made for a MOP goes on today's Tickets tray as it is made (storage's
// `link_mop`), so it shows on the Desk too.
import { api } from './api';
import { app, attempt, refresh, setView } from './store.svelte';
import { toast } from './toast.svelte';

/** Show `number` on the Tickets page. */
export async function goToTicket(number: string): Promise<void> {
  const t = number.trim();
  if (!t) return;
  const found = (await attempt(() => api.items([t]))) ?? [];
  const item = found[0];
  app.itemsFind = item?.number ?? t;
  app.itemsProject = 'all';
  app.itemsTab = item?.done ? 'done' : 'open';
  app.organiseOpen = false;
  setView('items');
  if (!item) toast(`No ticket ${t} yet.`);
}

/** Put `number` on today's Tickets tray, so it shows on the Desk; `quiet` leaves the toast
 *  to the caller. */
export async function addToToday(number: string, quiet = false): Promise<boolean> {
  const t = number.trim();
  if (!t) return false;
  const day = await attempt(() => api.trayAdd(app.today, t));
  if (!day) return false;
  await refresh();
  if (!quiet) toast(`${t} is on today's Tickets tray.`);
  return true;
}
