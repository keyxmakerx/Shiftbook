// MOPs on their change ticket (round 18): every MOP's run, so ticket cards and boxes can show
// "MOP · Ready", and the Attach a MOP picker. Loaded after unlock and on every change.
import { api } from './api';
import type { MopRun, MopStatus } from './api';
import { app, attempt } from './store.svelte';
import { openWindow } from './windows.svelte';

export const mops = $state<{ list: MopRun[]; attach: { ticket: string; title: string; templateId?: string } | null }>({
  list: [],
  attach: null,
});

export const STATUS_WORD: Record<MopStatus, string> = {
  draft: 'Draft',
  ready: 'Ready',
  running: 'Running',
  done: 'Done',
  backed_out: 'Backed out',
};

/** Ask for every MOP's run. Quiet on failure. */
export async function refreshMops(): Promise<void> {
  try {
    mops.list = await api.mopRuns(null);
  } catch {
    // The next change tries again.
  }
}

/** A MOP opens in a window big enough to run it. */
export function openMopWindow(id: string): void {
  openWindow('note', id, { w: 640, h: 560 });
}

/** The newest MOP on a ticket, if any (ignoring case). */
export function mopFor(ticket: string): MopRun | null {
  const t = ticket.trim().toLowerCase();
  return mops.list.find((r) => r.ticket.toLowerCase() === t) ?? null;
}

/** The ticket's MOP: open its newest one, or pick a template for a new one. `title` is what
 *  the work is called, for a template's {{title}} blank. */
export function openMop(ticket: string, title = ''): void {
  const run = mopFor(ticket);
  if (run) openMopWindow(run.mopId);
  else attachMop(ticket, title);
}

/** Pick a template (or page, or blank) for a new MOP on `ticket`. */
export function attachMop(ticket: string, title = '', templateId?: string): void {
  mops.attach = { ticket, title, templateId };
}

/** Make a MOP and open it in a window. */
export async function makeMop(
  ticket: string,
  from: { templateId?: string; pageId?: string; values?: [string, string][]; title?: string },
): Promise<void> {
  const note = await attempt(() =>
    api.attachMop(ticket, from.templateId ?? null, from.pageId ?? null, from.values ?? [], from.title ?? ''),
  );
  if (!note) return;
  mops.attach = null;
  await refreshMops();
  app.changed += 1;
  openMopWindow(note.id);
}
