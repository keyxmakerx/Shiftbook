// What you can do to one entry, shared by its row on Day and its card on the Desk (the
// Canvas): add time, move it to a project, recolour the project, delete, log it again, copy
// its tickets and bring it up tomorrow, plus the paper menu that offers all of them.
import { api } from './api';
import type { Entry } from './api';
import { followUpItem } from './bringUp';
import { sameWork } from './sameWork';
import type { MenuItem, MenuSpec } from './menu.svelte';
import { openNotepad } from './panel.svelte';
import { pinMenuItem } from './pins.svelte';
import { openWindow } from './windows.svelte';
import { copyText } from './protection.svelte';
import { app, attempt, flashEntry, projectById, putEntry, refresh, refreshProjects } from './store.svelte';
import { toast } from './toast.svelte';
import { repeating } from './repeating.svelte';
import { fmtMinutes } from './format';
import { startTimer, stopTimer, timer } from './timer.svelte';
import { startRename } from './rename.svelte';

/** The entry as an update with these changes (the project, so far). */
export function input(e: Entry, changes: Partial<{ projectId: string | null; recurringId: string | null }>) {
  return {
    date: e.date,
    minutes: e.minutes,
    description: e.description,
    projectId: changes.projectId !== undefined ? changes.projectId : e.projectId,
    recurringId: changes.recurringId !== undefined ? changes.recurringId : e.recurringId,
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
  // Time lands on the day being worked (today, or the day open on Day), never quietly on an
  // older day: rules in crates/core/src/quicklog.rs (plan_add_time).
  const day = app.view === 'day' ? app.selected : app.today;
  const done = await attempt(() => api.addEntryTime(entry.id, day, minutes));
  if (!done) return;
  const updated = done.entry;
  if (updated.date === app.selected) putEntry(updated);
  flashEntry(updated.id);
  void refresh();
  const where = updated.date === entry.date ? '' : updated.date === app.today ? ' today' : ` on ${updated.date}`;
  toast(`Added ${fmtMinutes(minutes)} to ${updated.description || 'the entry'}${where}`, 'Undo', () => {
    const undo: Promise<unknown> = done.created ? api.deleteEntry(updated.id) : api.addEntryMinutes(updated.id, -minutes);
    void attempt(() => undo).then(refresh);
  });
}

/**
 * Save the same change on each entry, then show them. Undo puts back what this changed (the
 * name, project and repeating thing) and keeps anything written since, like the notepad.
 */
async function changeAll(
  list: Entry[],
  change: (e: Entry) => ReturnType<typeof input>,
): Promise<(() => void) | null> {
  const pairs: { before: Entry; after: Entry }[] = [];
  for (const before of list) {
    const after = await attempt(() => api.updateEntry(before.id, change(before)));
    if (!after) break;
    pairs.push({ before, after });
  }
  if (!pairs.length) return null;
  for (const p of pairs) putEntry(p.after);
  void refresh();
  return () =>
    void Promise.all(
      pairs.map(({ before, after }) =>
        attempt(() =>
          api.updateEntry(after.id, {
            ...input(after, { projectId: before.projectId, recurringId: before.recurringId }),
            description: before.description,
          }),
        ),
      ),
    ).then(refresh);
}

/**
 * Give the entry a project (null for none), with Undo. `also` is the same work on other days
 * that its Desk card folds in: they move with it, so the card stays one card.
 */
export async function moveEntryTo(entry: Entry, projectId: string | null, also: readonly Entry[] = []): Promise<void> {
  const undo = await changeAll(sameWork(entry, also), (e) => input(e, { projectId, recurringId: null }));
  if (undo) toast(`Moved to ${projectById(projectId)?.name ?? 'No project'}`, 'Undo', undo);
}

/** File an entry under a repeating thing (round 15): it leaves its project. `also` as above. */
export async function moveEntryToRepeating(entry: Entry, recurringId: string, also: readonly Entry[] = []): Promise<void> {
  const undo = await changeAll(sameWork(entry, also), (e) => input(e, { projectId: null, recurringId }));
  if (!undo) return;
  const name = repeating.items.find((r) => r.id === recurringId)?.name ?? 'the repeating thing';
  toast(`Moved to ${name}`, 'Undo', undo);
}

/** Rename an entry and the same work folded into its Desk card (see `rename.svelte.ts`). */
export async function renameAll(entry: Entry, name: string, also: readonly Entry[] = []): Promise<boolean> {
  const next = name.trim();
  if (!next || next === entry.description) return false;
  const undo = await changeAll(sameWork(entry, also), (e) => ({ ...input(e, {}), description: next }));
  if (!undo) return false;
  toast(`Renamed to “${next}”`, 'Undo', undo);
  return true;
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
  }, () => void attempt(() => api.deleteEntry(id)).then(refresh));
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
  }, () => void attempt(() => api.restoreEntry(created.id)).then(refresh));
}

async function copyTickets(entry: Entry) {
  const text = entry.tickets.map((t) => t.number).join(', ') || entry.description;
  toast((await copyText(text)) ? 'Copied' : "Couldn't copy here.");
}

/**
 * The entry's paper menu. `also` is the same work its Desk card folds in (see `sameWork`).
 * About a dozen rows (the daily-use review, 10 Nov): the days, projects and colours open in
 * place under their own row, and the rarer things are under More.
 */
export function entryMenu(entry: Entry, also: readonly Entry[] = []): MenuSpec {
  const project = projectById(entry.projectId);
  const timing = timer.running && timer.target?.entryId === entry.id;
  const items: MenuItem[] = [
    { label: 'Rename', icon: 'edit', hint: 'F2', act: () => startRename(entry.id) },
    { label: 'Open details', icon: 'open', hint: 'Enter', act: () => (app.drawer = entry.id) },
    { label: entry.notes ? 'Open the notepad' : 'Write in the notepad', icon: 'edit', act: () => openNotepad(`entry:${entry.id}`) },
  ];
  if (timing) items.push({ label: 'Stop the timer', icon: 'clock', act: () => void stopTimer() });
  if (!entry.isTray) {
    items.push({
      label: 'Add time',
      icon: 'clock',
      sub: [
        {
          items: [
            { label: 'Add 15 minutes', icon: 'clock', act: () => void addEntryTime(entry, 15) },
            { label: 'Add 1 hour', icon: 'hour', act: () => void addEntryTime(entry, 60) },
            { label: 'Log again today (30 min)', icon: 'log', act: () => void logAgainToday(entry) },
          ],
        },
      ],
    });
  }
  items.push(followUpItem('entry', entry.id, entry.description));
  const choices = [
    ...app.projects
      .filter((p) => !p.archived)
      .map((p) => ({ label: p.name, color: p.color, checked: p.id === entry.projectId, act: () => void moveEntryTo(entry, p.id, also) })),
    { label: 'No project', color: null, checked: entry.projectId === null, act: () => void moveEntryTo(entry, null, also) },
  ];
  items.push({
    label: 'Project',
    icon: 'folder',
    value: project?.name ?? 'None',
    sub: [
      { choices },
      ...(project ? [{ head: `Colour for ${project.name}`, swatches: { current: project.color, pick: (c: string) => void setColor(entry, c) } }] : []),
    ],
  });
  const more: MenuItem[] = [
    { label: 'Pop out the notepad', icon: 'expand', act: () => openWindow('entry', entry.id) },
    pinMenuItem('entry', entry.id),
  ];
  if (canTime(entry) && !timing) more.push({ label: 'Start the timer', icon: 'clock', act: () => void startTimer(entry) });
  more.push({ label: 'Copy tickets', icon: 'copy', act: () => void copyTickets(entry) });
  items.push({ label: 'More', icon: 'more', sub: [{ items: more }] });
  return {
    sections: [
      { head: entry.description || 'Untitled', items },
      { items: [{ label: entry.isTray ? 'Delete (empties the tray)' : 'Delete', icon: 'trash', danger: true, act: () => void removeEntry(entry) }] },
    ],
  };
}
