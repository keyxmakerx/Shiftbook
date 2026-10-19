// What you can do to one ticket in the Tickets tray, shared by the tray on Day and the
// Desk's side panel and by ticket cards on the Desk: tick it done or keep it open, give it
// a project, carry it into today, take it out, plus the paper menu that offers all of them.
import { api } from './api';
import type { TrayDay, TrayItem } from './api';
import { bringUp } from './bringUp';
import { fmtMinutes } from './format';
import type { MenuItem, MenuSpec } from './menu.svelte';
import { copyText } from './protection.svelte';
import { app, attempt, flashEntry, projectById, refresh } from './store.svelte';
import { toast } from './toast.svelte';

/** Show a changed tray: the open day's tray updates, everything else reloads. */
export async function applyTray(day: TrayDay | undefined): Promise<void> {
  if (!day) return;
  if (app.tray?.date === day.date) app.tray = day;
  await refresh();
  if (day.entryId && day.date === app.selected) flashEntry(day.entryId);
}

export async function setTrayDone(item: TrayItem, done: boolean): Promise<void> {
  await applyTray(await attempt(() => api.traySetDone(item.id, done)));
  toast(
    done ? `${item.ticket} is done` : `${item.ticket} is still open. It comes back each day until you tick it.`,
    'Undo',
    () => void attempt(() => api.traySetDone(item.id, !done)).then(applyTray),
  );
}

export async function setTrayProject(item: TrayItem, projectId: string | null): Promise<void> {
  const before = item.projectId;
  if (before === projectId) return;
  await applyTray(await attempt(() => api.traySetProject(item.id, projectId)));
  const name = projectById(projectId)?.name;
  toast(name ? `${item.ticket}'s time now counts under ${name}` : `${item.ticket} is back in the day's Tickets entry`, 'Undo', () => {
    void attempt(() => api.traySetProject(item.id, before)).then(applyTray);
  });
}

/** Work on a still-open ticket again today: it joins today's tray with 15 minutes. */
export async function carryTray(item: TrayItem): Promise<void> {
  await applyTray(await attempt(() => api.trayCarry(item.id, app.selected)));
}

export async function removeTray(item: TrayItem): Promise<void> {
  await applyTray(await attempt(() => api.trayRemove(item.id)));
  toast(`Took ${item.ticket} out of the tray`, 'Undo', () => {
    void attempt(() => api.trayRestore(item.id)).then(applyTray);
  });
}

async function stepTray(item: TrayItem, by: number) {
  const next = Math.max(0, Math.min(480, item.minutes + by));
  if (next !== item.minutes) await applyTray(await attempt(() => api.traySetMinutes(item.id, next)));
}

async function copy(text: string, what: string) {
  toast((await copyText(text)) ? `Copied ${what}` : "Couldn't copy here.");
}

/** A tray ticket's paper menu. `notepad` opens its notepad where it is shown. */
export function trayMenu(item: TrayItem, notepad?: () => void): MenuSpec {
  const main: MenuItem[] = [
    item.done
      ? { label: 'Not done yet (keep it open)', icon: 'clock', side: true, act: () => void setTrayDone(item, false) }
      : { label: 'Done', icon: 'check', side: true, act: () => void setTrayDone(item, true) },
    { label: 'Bring up tomorrow', icon: 'sun', side: true, act: () => void bringUp('tray', item.id, item.ticket) },
  ];
  if (notepad) main.push({ label: item.note ? 'Open the notepad' : 'Write in the notepad', icon: 'edit', side: true, act: notepad });
  main.push(
    { label: 'Add 5 minutes', icon: 'plus', act: () => void stepTray(item, 5) },
    { label: 'Take away 5 minutes', icon: 'minus', act: () => void stepTray(item, -5) },
    { label: 'Copy ticket number', icon: 'copy', top: true, act: () => void copy(item.ticket, item.ticket) },
    ...item.links.map((l) => ({
      label: `Copy ${l.fieldName} link`,
      icon: 'link' as const,
      top: true,
      act: () => void copy(l.url, 'the link'),
    })),
  );
  const choices = [
    ...app.projects
      .filter((p) => !p.archived || p.id === item.projectId)
      .map((p) => ({ label: p.name, color: p.color, checked: p.id === item.projectId, act: () => void setTrayProject(item, p.id) })),
    { label: 'No project', color: null, checked: item.projectId === null, act: () => void setTrayProject(item, null) },
  ];
  return {
    sections: [
      { head: `${item.ticket} · ${fmtMinutes(item.minutes)}`, items: main },
      { head: 'Count its time under', choices },
      { items: [{ label: 'Take out of the tray', icon: 'trash', danger: true, top: true, act: () => void removeTray(item) }] },
    ],
  };
}
