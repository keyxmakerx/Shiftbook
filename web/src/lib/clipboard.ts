// Copying a secret (only the recovery key so far). The clipboard is cleared again after
// 60 seconds, and straight away when the app locks. Clipboard history or sync tools may
// still keep a copy, so the UI warns before copying.

export const CLEAR_AFTER_MS = 60 * 1000;

let clearTimer: ReturnType<typeof setTimeout> | null = null;

/** Copy `text`, and schedule the clipboard to be emptied. Resolves false if copying failed. */
export async function copySecret(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    return false;
  }
  if (clearTimer !== null) clearTimeout(clearTimer);
  clearTimer = setTimeout(() => {
    void clearCopiedSecret();
  }, CLEAR_AFTER_MS);
  return true;
}

/** True while a copied secret is waiting to be cleared. */
export function hasPendingClear(): boolean {
  return clearTimer !== null;
}

/** Empty the clipboard now if we put a secret there. */
export async function clearCopiedSecret(): Promise<void> {
  if (clearTimer === null) return;
  clearTimeout(clearTimer);
  clearTimer = null;
  try {
    await navigator.clipboard.writeText('');
  } catch {
    // Nothing more we can do; the warning told the user.
  }
}
