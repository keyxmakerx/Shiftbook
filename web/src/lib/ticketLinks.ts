// Opening a ticket's link in the user's own browser (round 21, card r21-open-links). Only
// links built from the link fields in Settings → Fields can be opened: the app checks the
// address against them before handing it over, so nothing typed in a note or made up by the
// page ever opens. Shiftbook itself still never goes online.
import { api } from './api';
import type { TicketLink } from './api';
import { openMenu, type MenuItem } from './menu.svelte';
import { attempt } from './store.svelte';
import { toast } from './toast.svelte';

export async function openTicketLink(ticket: string, link: TicketLink): Promise<void> {
  const ok = await attempt(() => api.openTicketLink(ticket, link.url).then(() => true));
  if (ok) toast(`Opened the ${link.fieldName} link for ${ticket} in your browser`);
}

/** Menu rows that open each of a ticket's links. */
export function openLinkItems(ticket: string, links: TicketLink[]): MenuItem[] {
  return links.map((l) => ({
    label: `Open ${l.fieldName} link`,
    icon: 'open' as const,
    act: () => void openTicketLink(ticket, l),
  }));
}

/** The ↗ beside a ticket: its one link opens straight away; several are offered in a menu. */
export function openLinks(anchor: HTMLElement, ticket: string, links: TicketLink[]): void {
  const [only, ...more] = links;
  if (!only) return;
  if (more.length === 0) void openTicketLink(ticket, only);
  else openMenu(anchor, { sections: [{ head: `Open ${ticket}`, items: openLinkItems(ticket, links) }] });
}
