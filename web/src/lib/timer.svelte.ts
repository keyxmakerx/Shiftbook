// The optional "Now doing" timer (Settings → Your day). One at a time; Stop offers to add
// the time to the entry, and nothing is added without a click. Locking keeps it as a
// paused timer (the entry and counts of seconds only), picked up after unlock.
import { api } from './api';
import type { Entry, TimerOffer } from './api';
import { addDays } from './format';
import { onBeforeLock } from './lock';
import { app, attempt, flashEntry, refresh } from './store.svelte';
import { noticeActivity, seconds, startClock, tickClock, type Clock } from './timerClock';
import { toast } from './toast.svelte';

interface Target {
  entryId: string;
  title: string;
}

export const timer = $state({
  /** The entry being timed, or null when there's no timer. */
  target: null as Target | null,
  running: false,
  shown: 0,
  /** Set after Stop, until the user picks what to add. */
  offer: null as TimerOffer | null,
  /** Asked to start on another entry while one ran: start it once the offer is settled. */
  next: null as Target | null,
});

let clock: Clock = startClock(Date.now());
let ticker: ReturnType<typeof setInterval> | null = null;

function run() {
  clock = startClock(Date.now(), clock.counted, clock.quiet);
  timer.running = true;
  ticker ??= setInterval(() => {
    clock = tickClock(clock, Date.now());
    timer.shown = seconds(clock);
  }, 1000);
}

function halt() {
  if (ticker) clearInterval(ticker);
  ticker = null;
  if (timer.running) clock = tickClock(clock, Date.now());
  timer.shown = seconds(clock);
  timer.running = false;
}

function clear() {
  halt();
  clock = startClock(Date.now());
  timer.target = null;
  timer.offer = null;
  timer.shown = 0;
}

/** Called on keyboard or mouse activity, to notice time away while running. */
export function timerActivity(): void {
  if (timer.running) clock = noticeActivity(clock, Date.now());
}

export function entryTitle(e: Entry): string {
  return e.description.trim() || e.tickets.map((t) => t.number).join(', ') || 'this entry';
}

/** ▶ Start on an entry. With another timer running, stop that one first. */
export async function startTimer(entry: Entry): Promise<void> {
  const target = { entryId: entry.id, title: entryTitle(entry) };
  if (timer.target && timer.target.entryId !== entry.id) {
    timer.next = target;
    await stopTimer();
    return;
  }
  if (!timer.target) {
    clock = startClock(Date.now());
    timer.shown = 0;
  }
  timer.target = target;
  timer.offer = null;
  run();
  await clearPaused();
}

/** Stop and ask what to add. */
export async function stopTimer(): Promise<void> {
  if (!timer.target) return;
  halt();
  const offer = await attempt(() => api.timerOffer(seconds(clock), clock.quiet));
  if (offer) timer.offer = offer;
}

/** Keep counting after Stop (the offer goes away). */
export function resumeTimer(): void {
  if (!timer.target) return;
  timer.offer = null;
  timer.next = null;
  run();
  void clearPaused();
}

/** Add the chosen minutes to the entry (0 adds nothing), then start any waiting timer. */
export async function settleTimer(minutes: number): Promise<void> {
  const target = timer.target;
  if (!target) return;
  if (minutes > 0) {
    const updated = await attempt(() => api.addEntryMinutes(target.entryId, minutes));
    if (!updated) return;
    await refresh();
    flashEntry(updated.id);
    toast(`Added ${minutes} minutes to “${target.title}”.`);
  }
  clear();
  await clearPaused();
  const next = timer.next;
  timer.next = null;
  if (next) {
    timer.target = next;
    clock = startClock(Date.now());
    run();
  }
}

async function clearPaused(): Promise<void> {
  if (app.prefs?.pausedTimer) {
    const p = await attempt(() => api.savePref('timer.paused', ''));
    if (p) app.prefs = p;
  }
}

/** After unlock: pick up a timer paused by locking, paused (press ▶ to go on). */
export async function restoreTimer(): Promise<void> {
  const paused = app.prefs?.pausedTimer;
  if (!paused || timer.target) return;
  const recent = await api.listEntries(addDays(app.today, -14), app.today).catch(() => [] as Entry[]);
  const entry = recent.find((e) => e.id === paused.entryId);
  if (!entry) {
    await clearPaused();
    return;
  }
  clock = startClock(Date.now(), paused.seconds * 1000, paused.quiet);
  timer.shown = paused.seconds;
  timer.target = { entryId: entry.id, title: entryTitle(entry) };
  timer.running = false;
}

// Locking keeps the count as a paused timer.
onBeforeLock(async () => {
  if (!timer.target) return;
  halt();
  const quiet = clock.quiet.slice(-100).join(',');
  await api.savePref('timer.paused', `${timer.target.entryId} ${Math.min(seconds(clock), 86_400)} ${quiet}`.trim());
});
