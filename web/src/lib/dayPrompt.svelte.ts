// "Pick a day…" (round 15): a small box asking for a date, used by "Bring up…". One at a
// time; `askDay` resolves with the ISO date picked, or null when it's closed.

export const dayPrompt = $state({ open: false, title: '', min: '', serial: 0 });

let settle: ((day: string | null) => void) | null = null;

/** Ask for a day on or after `min`. */
export function askDay(title: string, min: string): Promise<string | null> {
  settle?.(null);
  dayPrompt.title = title;
  dayPrompt.min = min;
  dayPrompt.open = true;
  dayPrompt.serial += 1;
  return new Promise((resolve) => {
    settle = resolve;
  });
}

/** Close the box with the day picked, or null. */
export function answerDay(day: string | null): void {
  dayPrompt.open = false;
  const done = settle;
  settle = null;
  done?.(day);
}
