// The saved responses menu (round 8), shared by the Tickets tray's Responses button and, since
// round 21, the "Copy a response" button on the message that says a ticket is done. Picking
// one copies it with its blanks filled in: {ticket} when the menu was opened for a ticket,
// {date} always, and {name} after asking for the name once.
import { api } from './api';
import { askText } from './askText.svelte';
import { openMenu } from './menu.svelte';
import { copyText } from './protection.svelte';
import { app, attempt, setView } from './store.svelte';
import { toast } from './toast.svelte';

function editResponses(): void {
  app.settingsSection = 'responses';
  setView('settings');
}

async function copyResponse(id: string, ticket: string | null): Promise<void> {
  const list = await attempt(() => api.responsesFor(ticket, app.today, null));
  let picked = list?.find((r) => r.id === id);
  if (!picked) return;
  if (picked.asksName) {
    const name = await askText({
      title: 'Who is it to?',
      label: 'Their name',
      hint: `Fills in {name} in “${picked.name}”.`,
      ok: 'Copy',
    });
    if (name === null) return;
    picked = (await attempt(() => api.responsesFor(ticket, app.today, name)))?.find((r) => r.id === id);
    if (!picked) return;
  }
  const name = picked.name;
  const ok = await copyText(picked.text);
  toast(ok ? (ticket ? `Copied “${name}” for ${ticket}` : `Copied “${name}”`) : "Couldn't copy here.");
}

/** Open the menu under `anchor`. With a `ticket`, its number goes into each response. */
export async function openResponses(anchor: HTMLElement, ticket: string | null = null): Promise<void> {
  const list = await attempt(() => api.responsesFor(ticket, app.today, null));
  if (!list) return;
  openMenu(anchor, {
    sections: [
      list.length > 0
        ? {
            head: ticket ? `Copy a response for ${ticket}` : 'Copy a response',
            items: list.map((r) => ({ label: r.name, icon: 'copy' as const, act: () => void copyResponse(r.id, ticket) })),
          }
        : { head: 'No saved responses yet', items: [{ label: 'Add your first response', icon: 'plus', act: editResponses }] },
      { items: [{ label: 'Edit responses', icon: 'edit', act: editResponses }] },
    ],
  });
}

/** The second button for a "… is done" message: Copy a response, for that ticket. */
export function responseButton(ticket: string): { label: string; run: (button: HTMLElement) => void } {
  // The menu hangs from the whole message, which lifts cleanly; the button alone is see-through.
  return { label: 'Copy a response', run: (button) => void openResponses(button.closest<HTMLElement>('.toast') ?? button, ticket) };
}
