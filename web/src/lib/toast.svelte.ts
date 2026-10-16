// One toast at a time at the bottom of the window, with an optional Undo-style button.
// A toast with a button stays 10 seconds, and waits while the pointer or focus is on it.

export const toastState = $state({ text: '', action: '', on: false, serial: 0 });

/** How long a plain toast stays. */
export const PLAIN_MS = 4800;
/** How long a toast with a button (Undo) stays. */
export const ACTION_MS = 10_000;
/** Time left after the pointer or focus leaves a toast, at least. */
const AFTER_HOLD_MS = 3000;

let handler: (() => void) | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let endsAt = 0;
let held = false;

function schedule(ms: number): void {
  if (timer !== null) clearTimeout(timer);
  endsAt = Date.now() + ms;
  timer = setTimeout(() => {
    timer = null;
    toastState.on = false;
  }, ms);
}

export function toast(text: string, action = '', run: (() => void) | null = null): void {
  toastState.text = text;
  toastState.action = run ? action : '';
  toastState.on = true;
  toastState.serial += 1;
  handler = run;
  held = false;
  schedule(run ? ACTION_MS : PLAIN_MS);
}

/** The pointer or focus is on the toast: stop the clock. */
export function holdToast(): void {
  if (!toastState.on || held) return;
  held = true;
  if (timer !== null) clearTimeout(timer);
  timer = null;
}

/** The pointer or focus left: carry on with what was left, but at least a few seconds. */
export function releaseToast(): void {
  if (!toastState.on || !held) return;
  held = false;
  schedule(Math.max(endsAt - Date.now(), AFTER_HOLD_MS));
}

export function runToastAction(): void {
  const run = handler;
  handler = null;
  held = false;
  toastState.on = false;
  run?.();
}
