// What you can do to one entry, shared by its row on Day and its card on the Desk (the
// Canvas): add time, move it to a project, recolour the project, delete, log it again, copy
// its tickets and bring it up tomorrow, plus the paper menu that offers all of them.
import { api } from './api';
import type { Entry } from './api';
import { bringUp } from './bringUp';
import type { MenuItem, MenuSpec } from './menu.svelte';
import { openNotepad } from './panel.svelte';
import { openWindow } from './windows.svelte';
import { copyText } from './protection.svelte';
import { app, attempt, flashEntry, projectById, putEntry, refresh, refreshProjects } from './store.svelte';
import { toast } from './toast.svelte';
import { fmtMinutes } from './format';
import { startTimer, stopTimer, timer } from './timer.svelte';

/** The entry as an update with these changes (the project, so far). */
export function input(e: Entry, changes: Partial<{ projectId: string | null }>) {
  return {
    date: e.date,
    minutes: e.minutes,
    description: e.description,
    projectId: changes.projectId !== undefined ? changes.projectId : e.projectId,
    category: e.category,
    notes: e.notes,
    timesheetCodeId: e.timesheetCodeId,
  };
}

/** The optional timer (Settings → Your day) runs on today's entries only. */
export function canTime(entry: Entry): boolean {
  return !!app.prefs?.timer && !entry.isTray && entry.date === app.today;
}

export async function addEntryTime(entry: Entry, minutes: number): Promise<void> {
  if (entry.isTray) {
    toast('The Tickets entry adds up the Tickets tray. Change minutes there.');
    return;
  }
  const updated = await attempt(() => api.addEntryMinutes(entry.id, minutes));
  if (!updated) return;
  putEntry(updated);
  flashEntry(updated.id);
  void refresh();
  toast(`Added ${fmtMinutes(minutes)} to ${updated.description || 'the entry'}`, 'Undo', () => {
    void attempt(() => api.addEntryMinutes(updated.id, -minutes)).then(refresh);
  });
}

/** Give the entry a project (null for none), with Undo. */
export async function moveEntryTo(entry: Entry, projectId: string | null): Promise<void> {
  const before = entry.projectId;
  const updated = await attempt(() => api.updateEntry(entry.id, input(entry, { projectId })));
  if (!updated) return;
  putEntry(updated);
  void refresh();
  toast(`Moved to ${projectById(projectId)?.name ?? 'No project'}`, 'Undo', () => {
    void attempt(() => api.updateEntry(updated.id, input(updated, { projectId: before }))).then(refresh);
  });
}

async function setColor(entry: Entry, c: string) {
  const p = projectById(entry.projectId);
  if (!p) return;
  const before = p.color;
  const saved = await attempt(() =>
    api.updateProject(p.id, { name: p.name, color: c, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: p.archived }),
  );
  if (!saved) return;
  await refreshProjects();
  toast(`${p.name} is now this colour everywhere`, 'Undo', () => {
    void attempt(() =>
      api.updateProject(p.id, { name: p.name, color: before, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: p.archived }),
    ).then(refreshProjects);
  });
}

export async function removeEntry(entry: Entry): Promise<void> {
  const id = entry.id;
  const title = entry.description || 'the entry';
  const ok = await attempt(async () => {
    await api.deleteEntry(id);
    return true;
  });
  if (!ok) return;
  app.drawer = app.drawer === id ? null : app.drawer;
  await refresh();
  toast(`Deleted “${title}”`, 'Undo', () => {
    void attempt(() => api.restoreEntry(id)).then(refresh);
  });
}

async function logAgainToday(entry: Entry) {
  const created = await attempt(() =>
    api.createEntry(
      { ...input(entry, {}), date: app.today, minutes: 30 },
      entry.tickets.map((t) => t.number).join('\n'),
    ),
  );
  if (!created) return;
  await refresh();
  flashEntry(created.id);
  toast(`Logged 0.5 h of “${created.description}” today`, 'Undo', () => {
    void attempt(() => api.deleteEntry(created.id)).then(refresh);
  });
}

async function copyTickets(entry: Entry) {
  const text = entry.tickets.map((t) => t.number).join(', ') || entry.description;
  toast((await copyText(text)) ? 'Copied' : "Couldn't copy here.");
}

/** The entry's paper menu. */
export function entryMenu(entry: Entry): MenuSpec {
  const project = projectById(entry.projectId);
  const timing = timer.running && timer.target?.entryId === entry.id;
  const main: MenuItem[] = [
    { label: 'Open details', icon: 'open', side: true, hint: 'Enter', act: () => (app.drawer = entry.id) },
    { label: entry.notes ? 'Open the notepad' : 'Write in the notepad', icon: 'edit', side: true, act: () => openNotepad(`entry:${entry.id}`) },
    { label: 'Open the notepad in a window', icon: 'expand', act: () => openWindow('entry', entry.id) },
    { label: 'Bring up tomorrow', icon: 'sun', side: true, act: () => void bringUp('entry', entry.id, entry.description) },
  ];
  if (!entry.isTray) {
    main.push(
      { label: 'Add 15 minutes', icon: 'clock', side: true, act: () => void addEntryTime(entry, 15) },
      { label: 'Add 1 hour', icon: 'hour', side: true, act: () => void addEntryTime(entry, 60) },
    );
  }
  if (canTime(entry)) {
    main.push(
      timing
        ? { label: 'Stop the timer', icon: 'clock', top: true, act: () => void stopTimer() }
        : { label: 'Start the timer', icon: 'clock', top: true, act: () => void startTimer(entry) },
    );
  }
  main.push(
    { label: 'Copy tickets', icon: 'copy', top: true, act: () => void copyTickets(entry) },
    { label: 'Log again today (30 min)', icon: 'log', top: true, act: () => void logAgainToday(entry) },
  );
  const choices = [
    ...app.projects
      .filter((p) => !p.archived)
      .map((p) => ({ label: p.name, color: p.color, checked: p.id === entry.projectId, act: () => void moveEntryTo(entry, p.id) })),
    { label: 'No project', color: null, checked: entry.projectId === null, act: () => void moveEntryTo(entry, null) },
  ];
  const sections: MenuSpec['sections'] = [
    { head: entry.description || 'Untitled', items: main },
    { head: 'Project', choices },
  ];
  if (project) {
    sections.push({ head: `Colour for ${project.name}`, swatches: { current: project.color, pick: (c) => void setColor(entry, c) } });
  }
  sections.push({
    items: [{ label: entry.isTray ? 'Delete (empties the tray)' : 'Delete', icon: 'trash', danger: true, top: true, act: () => void removeEntry(entry) }],
  });
  return { sections };
}
