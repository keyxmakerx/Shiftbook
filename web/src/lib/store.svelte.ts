// The unlocked app's shared state. Everything comes from DataApi; numbers and rules are
// computed by the app (crates/core), this file only keeps the latest answers.

import { api } from './api';
import type {
  CustomField,
  Entry,
  InboxItem,
  PrefKey,
  ResumeCard,
  ResumeTrigger,
  Prefs,
  Project,
  RibbonDay,
  RibbonWeek,
  TimesheetCode,
  TrayDay,
} from './api';
import { toIso } from './format';
import type { SettingsSection, View } from './keys';
import { errorMessage } from './messages';
import { setMotion } from './motion';
import { toast } from './toast.svelte';

export const app = $state({
  ready: false,
  today: toIso(new Date()),
  selected: toIso(new Date()),
  view: 'desk' as View,
  /** The view we came from, for the direction of the slide. */
  previousView: 'desk' as View,
  prefs: null as Prefs | null,
  projects: [] as Project[],
  codes: [] as TimesheetCode[],
  fields: [] as CustomField[],
  ribbon: [] as RibbonWeek[],
  entries: [] as Entry[],
  tray: null as TrayDay | null,
  inbox: [] as InboxItem[],
  /** Entry open in the details drawer. */
  drawer: null as string | null,
  captureOpen: false,
  /** Bumped to ask Log work to take focus, with optional text to put in it. */
  logFocus: { serial: 0, text: '' },
  /** The date the wrap-up was finished or put off, so it stays away until tomorrow. */
  wrapupClosed: '',
  /** Entry to flash after it changed. */
  flash: { id: '', serial: 0 },
  /** The "Where was I?" card on the Desk, while it's showing. */
  resume: null as ResumeCard | null,
  /** "Later" was pressed: no more "Where was I?" until the next unlock. */
  resumeLater: false,
  /** The Settings section to open next time Settings shows. */
  settingsSection: 'look' as SettingsSection,
});

/** Run an API call; on failure show a plain message and resolve undefined. */
export async function attempt<T>(work: () => Promise<T>): Promise<T | undefined> {
  try {
    return await work();
  } catch (e) {
    toast(errorMessage(e));
    return undefined;
  }
}

export function projectById(id: string | null | undefined): Project | undefined {
  return id ? app.projects.find((p) => p.id === id) : undefined;
}

export function projectColor(id: string | null | undefined): string {
  return projectById(id)?.color ?? 'var(--none)';
}

export function projectName(id: string | null | undefined): string {
  return projectById(id)?.name ?? 'No project';
}

export function ribbonDay(iso: string): RibbonDay | undefined {
  for (const week of app.ribbon) {
    const day = week.days.find((d) => d.date === iso);
    if (day) return day;
  }
  return undefined;
}

export function ribbonWeekOf(iso: string): RibbonWeek | undefined {
  return app.ribbon.find((w) => w.days.some((d) => d.date === iso));
}

export function flashEntry(id: string): void {
  app.flash = { id, serial: app.flash.serial + 1 };
}

/** Apply theme, motion and accent to the page. */
export function applyPrefs(prefs: Prefs): void {
  const root = document.documentElement;
  if (prefs.theme === 'system') delete root.dataset.theme;
  else root.dataset.theme = prefs.theme;
  root.dataset.motion = prefs.motion;
  root.style.setProperty('--m', prefs.motion === 'lively' ? '1.35' : '1');
  if (prefs.accent) root.style.setProperty('--accent', prefs.accent);
  else root.style.removeProperty('--accent');
  // Text size is applied by the desktop app as webview zoom when prefs load or change.
  setMotion(prefs.motion);
}

/** Save one preference and apply the result. */
export async function savePref(key: PrefKey, value: string): Promise<Prefs | undefined> {
  const p = await attempt(() => api.savePref(key, value));
  if (!p) return undefined;
  app.prefs = p;
  applyPrefs(p);
  return p;
}

export async function refreshDay(): Promise<void> {
  const [entries, tray] = await Promise.all([
    api.listEntries(app.selected, app.selected),
    api.trayDay(app.selected),
  ]);
  app.entries = entries;
  app.tray = tray;
}

export async function refreshRibbon(): Promise<void> {
  app.ribbon = await api.ribbon(app.today);
}

/** Reload the selected day and the ribbon after a change. */
export async function refresh(): Promise<void> {
  await attempt(() => Promise.all([refreshDay(), refreshRibbon()]));
}

export async function refreshProjects(): Promise<void> {
  app.projects = await api.listProjects();
}

export async function refreshInbox(): Promise<void> {
  app.inbox = await api.listInbox();
}

export async function loadAll(): Promise<void> {
  app.today = toIso(new Date());
  const [prefs, projects, codes, fields, inbox] = await Promise.all([
    api.loadPrefs(),
    api.listProjects(),
    api.listTimesheetCodes(),
    api.listCustomFields(),
    api.listInbox(),
  ]);
  app.prefs = prefs;
  applyPrefs(prefs);
  app.projects = projects;
  app.codes = codes;
  app.fields = fields;
  app.inbox = inbox;
  await Promise.all([refreshDay(), refreshRibbon()]);
  app.ready = true;
}

export function setView(view: View): void {
  if (view === app.view) return;
  app.previousView = app.view;
  app.view = view;
  app.drawer = null;
}

export async function selectDay(iso: string): Promise<void> {
  app.selected = iso > app.today ? app.today : iso;
  await attempt(refreshDay);
}

/** Replace one entry in the day list after it changed (or add it). */
export function putEntry(entry: Entry): void {
  if (entry.date !== app.selected) return;
  const i = app.entries.findIndex((e) => e.id === entry.id);
  if (i >= 0) app.entries[i] = entry;
  else app.entries.push(entry);
}

export function focusLog(text = ''): void {
  app.view = 'desk';
  app.logFocus = { serial: app.logFocus.serial + 1, text };
}

/** Ask whether "Where was I?" should show for this return, and show it. */
export async function checkResume(trigger: ResumeTrigger, awayMinutes = 0): Promise<void> {
  if (app.resumeLater) return;
  const card = await api.whereWasI(app.today, trigger, awayMinutes).catch(() => null);
  if (card && !app.resumeLater) app.resume = card;
}

/** Move to the new date when the day changes while Shiftbook is open. True if it changed. */
export async function rollDay(): Promise<boolean> {
  const now = toIso(new Date());
  if (now === app.today) return false;
  const wasToday = app.selected === app.today;
  app.today = now;
  if (wasToday) app.selected = now;
  await refresh();
  return true;
}
