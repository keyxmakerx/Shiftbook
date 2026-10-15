// One toast at a time at the bottom of the window, with an optional Undo-style button.

export const toastState = $state({ text: '', action: '', on: false, serial: 0 });

let handler: (() => void) | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;

export function toast(text: string, action = '', run: (() => void) | null = null): void {
  toastState.text = text;
  toastState.action = run ? action : '';
  toastState.on = true;
  toastState.serial += 1;
  handler = run;
  if (timer !== null) clearTimeout(timer);
  timer = setTimeout(() => {
    toastState.on = false;
  }, 4800);
}

export function runToastAction(): void {
  const run = handler;
  handler = null;
  toastState.on = false;
  run?.();
}
