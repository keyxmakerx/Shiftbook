// The optional "Now doing" timer (Settings → Your day). One at a time; Stop offers to add
// the time to the entry, and nothing is added without a click. Locking keeps it as a
// paused timer (the entry and counts of seconds only), picked up after unlock.
import { api } from './api';
import type { Entry, TimerOffer } from './api';
import { addDays } from './format';
import { onBeforeLock } from './lock';
import { phone } from './phone.svelte';
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
  /** Phone: back after a long time away with the timer running; asks whether to count it. */
  away: null as { seconds: number; minutes: number } | null,
});

/** Phone: the timer was running when Shiftbook went to the background. Kept across the
 *  reload that locking does (a flag only: no times). */
const AWAY_FLAG = 'sb.timerAway';

function awayFlag(on?: boolean): boolean {
  try {
    if (on === true) sessionStorage.setItem(AWAY_FLAG, '1');
    else if (on === false) sessionStorage.removeItem(AWAY_FLAG);
    return sessionStorage.getItem(AWAY_FLAG) === '1';
  } catch {
    return false;
  }
}

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
  timer.away = null;
  clock = startClock(Date.now());
  timer.target = null;
  timer.offer = null;
  timer.shown = 0;
}

/** Phone: Shiftbook went to the background. Counting stops; Android measures the time away
 *  and shows the timer in a notification and the tile meanwhile. */
export function timerHidden(): void {
  if (!timer.running) return;
  halt();
  awayFlag(true);
  void api.phoneTimerAway(true, seconds(clock)).catch(() => undefined);
}

/** Phone: back (or unlocked after locking while away). Short times away are counted; from
 *  five minutes the timer asks (crates/core/src/timer.rs, away_question). */
export async function timerBack(): Promise<void> {
  if (!awayFlag()) return;
  awayFlag(false);
  const away = await api.phoneAwaySeconds().catch(() => 0);
  void api.phoneTimerAway(false, 0).catch(() => undefined);
  if (!timer.target || timer.offer) return;
  clock = { ...clock, counted: clock.counted + away * 1000 };
  timer.shown = seconds(clock);
  const minutes = await api.timerAwayQuestion(away).catch(() => null);
  timer.away = minutes === null ? null : { seconds: away, minutes };
  run();
  void clearPaused();
}

/** Phone: "Leave it out" takes the time away off again; "Count it" keeps it. */
export function settleAway(count: boolean): void {
  const away = timer.away;
  if (!away) return;
  timer.away = null;
  if (count) return;
  clock = { ...clock, counted: Math.max(0, clock.counted - away.seconds * 1000) };
  timer.shown = seconds(clock);
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
  // Phone: the timer shows in a notification while Shiftbook is in the background, which
  // Android 13 on asks permission for. Asked once, the first time a timer starts.
  if (phone.on && !askedToNotify) {
    askedToNotify = true;
    void api.phoneAllowNotifications().catch(() => false);
  }
}

let askedToNotify = false;

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
  // Phone: locked while away with the timer running, so it goes on and asks about the time away.
  await timerBack();
}

// Locking keeps the count as a paused timer.
onBeforeLock(async () => {
  if (!timer.target) return;
  halt();
  const quiet = clock.quiet.slice(-100).join(',');
  await api.savePref('timer.paused', `${timer.target.entryId} ${Math.min(seconds(clock), 86_400)} ${quiet}`.trim());
});
