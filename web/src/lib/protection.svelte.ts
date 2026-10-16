// Screenshot and copy protection (decided in rounds 4 and 5).
//
// On by default: the app window is hidden from screenshots and screen shares (Windows,
// partly macOS; Linux can't) and copying or cutting text out of the page is blocked. The
// pill allows both for ten minutes; a timer in the app turns protection back on. The
// explicit Copy buttons always work because they go through `copyText`.

import { api } from './api';
import { toast } from './toast.svelte';

export const protection = $state({ on: true, secondsLeft: 0 });

let bypass = false;
let ticker: ReturnType<typeof setInterval> | null = null;
let until = 0;

function stopTicker(): void {
  if (ticker !== null) clearInterval(ticker);
  ticker = null;
}

function guard(event: ClipboardEvent): void {
  if (!protection.on || bypass) return;
  event.preventDefault();
  toast('Copying is blocked while Shiftbook is protected. Use a Copy button, or allow it for 10 minutes.');
}

/** Block copy and cut while protected. Returns a function that removes the guard. */
export function installCopyGuard(): () => void {
  document.addEventListener('copy', guard, true);
  document.addEventListener('cut', guard, true);
  return () => {
    document.removeEventListener('copy', guard, true);
    document.removeEventListener('cut', guard, true);
  };
}

/** Allow screenshots and copying for the time the app grants (10 minutes). */
export async function allowCapture(): Promise<void> {
  const seconds = await api.allowCapture();
  protection.on = false;
  until = Date.now() + seconds * 1000;
  protection.secondsLeft = seconds;
  stopTicker();
  ticker = setInterval(() => {
    const left = Math.max(0, Math.round((until - Date.now()) / 1000));
    protection.secondsLeft = left;
    if (left === 0) {
      stopTicker();
      protection.on = true;
    }
  }, 1000);
}

/** Protect again now. */
export async function protectNow(): Promise<void> {
  stopTicker();
  protection.on = true;
  protection.secondsLeft = 0;
  await api.protectCapture();
}

/** "9:58". */
export function countdown(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s < 10 ? '0' : ''}${s}`;
}

/** Copy text from an explicit Copy button: works even while protected. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Older WebViews: copy through a hidden text box, letting this one copy through.
    const box = document.createElement('textarea');
    box.value = text;
    box.setAttribute('readonly', '');
    box.className = 'visually-hidden';
    document.body.appendChild(box);
    box.select();
    bypass = true;
    try {
      return document.execCommand('copy');
    } catch {
      return false;
    } finally {
      bypass = false;
      box.remove();
    }
  }
}
