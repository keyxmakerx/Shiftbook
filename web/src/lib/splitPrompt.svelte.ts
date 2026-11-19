// "Split in two…" (round 21): a box asking how an entry's time is shared between two parts.
// One at a time; `askSplit` resolves with the answer, or null when it's closed.
import type { Entry, SplitInput } from './api';

export const splitPrompt = $state<{ open: boolean; entry: Entry | null; serial: number }>({ open: false, entry: null, serial: 0 });

let settle: ((split: SplitInput | null) => void) | null = null;

/** Ask how to split `entry`. */
export function askSplit(entry: Entry): Promise<SplitInput | null> {
  settle?.(null);
  splitPrompt.entry = entry;
  splitPrompt.open = true;
  splitPrompt.serial += 1;
  return new Promise((resolve) => {
    settle = resolve;
  });
}

/** Close the box with the split, or null. */
export function answerSplit(split: SplitInput | null): void {
  splitPrompt.open = false;
  const done = settle;
  settle = null;
  done?.(split);
}

/** Where the box starts: half the time to the nearest 5 minutes, leaving both parts some. */
export function splitStart(total: number): number {
  const half = Math.floor((Math.floor(total / 2) + 2) / 5) * 5;
  return Math.min(total - 1, Math.max(1, half));
}
