// What the unlocked app does while it's open, shared by the computer layout (pages/Shell)
// and the phone layout (pages/PhoneShell): load everything, notice returns for "Where was I?",
// roll over at midnight, keep the bell and pins fresh, and guard copying.

import { untrack } from 'svelte';
import { openAway, refreshBell } from './bell.svelte';
import { errorMessage } from './messages';
import { refreshPins } from './pins.svelte';
import { installCopyGuard } from './protection.svelte';
import { noticeReturn } from './resume';
import { restoreTimer, timerActivity } from './timer.svelte';
import { app, checkResume, loadAll, rollDay } from './store.svelte';
import { loadWindows } from './windows.svelte';
import { loadMopBook } from './mopbook.svelte';

/** Call from a component's `onMount`; returns the cleanup. */
export function startShell(onProblem: (message: string) => void): () => void {
  loadAll()
    .then(() => {
      loadWindows();
      loadMopBook();
      return Promise.all([checkResume('unlocked'), restoreTimer(), refreshBell(), refreshPins(), openAway()]);
    })
    .catch((e: unknown) => onProblem(errorMessage(e)));
  const unguard = installCopyGuard();

  // "Where was I?" after a quiet stretch or when the day changes while open.
  let last = Date.now();
  const onActivity = () => {
    timerActivity();
    const seen = noticeReturn(last, Date.now());
    last = seen.lastActivity;
    if (seen.awayMinutes !== null && app.ready) void checkResume('break', seen.awayMinutes);
  };
  const events = ['keydown', 'pointerdown', 'pointermove', 'wheel'] as const;
  for (const name of events) window.addEventListener(name, onActivity, { passive: true });
  const timer = setInterval(async () => {
    if (app.ready && (await rollDay())) {
      void checkResume('new_day');
      void openAway();
    }
    if (app.ready) void refreshBell();
    if (app.ready) void refreshPins();
  }, 60_000);
  return () => {
    clearInterval(timer);
    for (const name of events) window.removeEventListener(name, onActivity);
    unguard();
  };
}

/** The bell follows the day's changes (a ticket ticked, something brought up). Call during
 *  component setup. */
export function followDay(): void {
  $effect(() => {
    void app.tray;
    void app.entries;
    if (app.ready) untrack(() => void Promise.all([refreshBell(), refreshPins()]));
  });
}
