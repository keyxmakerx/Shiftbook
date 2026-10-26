// Phone (round 15): what to do when Android opens Shiftbook for something — text shared into
// it (idea C), a long-press shortcut (D), the timer tile (F) or a nudge (G, due reminders) —
// and planning the next nudges. Always after unlock: Android hands the reason over only to
// the unlocked app, which asks for it here.
import { api } from './api';
import { bell, nowMinutes } from './bell.svelte';
import { phone } from './phone.svelte';
import { app, openNote, setView } from './store.svelte';
import { timer } from './timer.svelte';
import { toast } from './toast.svelte';

export const launch = $state({
  /** Text shared into Shiftbook, waiting on the share sheet. */
  share: null as string | null,
  /** Capture opens as "start the timer on this". */
  timerCapture: false,
  /** Capture opens with this already typed. */
  prefill: '',
});

/** Open capture, optionally with text typed or as "start the timer". */
export function openCapture(prefill = '', forTimer = false): void {
  launch.prefill = prefill;
  launch.timerCapture = forTimer;
  app.captureOpen = true;
}

/** Ask Android why it opened Shiftbook, and do that. Quiet when there's nothing. */
export async function takeLaunch(): Promise<void> {
  if (!phone.on || !app.ready) return;
  const got = await api.phoneLaunch().catch(() => null);
  if (!got) return;
  switch (got.kind) {
    case 'share':
      launch.share = got.text;
      break;
    case 'log':
      setView('desk');
      openCapture();
      break;
    case 'jot':
      openNote(null);
      break;
    case 'timer':
      setView('desk');
      if (timer.target) break;
      if (!app.prefs?.timer) {
        toast('The timer is off. Turn it on in More → Your day.');
        break;
      }
      openCapture('', true);
      break;
    case 'due':
      bell.open = true;
      break;
    case 'wrapup':
      setView('desk');
      break;
  }
}

/** Plan the next due reminder and wrap-up nudge (or clear them when both are off). */
export async function planNudges(): Promise<void> {
  if (!phone.on || !app.ready) return;
  await api.planNudges(app.today, nowMinutes()).catch(() => undefined);
}
