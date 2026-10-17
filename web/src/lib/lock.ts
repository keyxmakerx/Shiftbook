import { api } from './api';
import { clearCopiedSecret } from './clipboard';

const beforeLock: (() => Promise<void>)[] = [];

/** Run `save` just before locking (for example to keep a running timer as paused). */
export function onBeforeLock(save: () => Promise<void>): void {
  beforeLock.push(save);
}

/**
 * Lock now: empty the clipboard if we copied a secret, ask the app to close the data
 * file and forget the key, then reload the page so nothing decrypted stays in memory.
 * The page reloads even if the lock call fails; the unlock screen shows the real state.
 */
export async function lockAndReload(): Promise<void> {
  for (const save of beforeLock) await save().catch(() => undefined);
  await clearCopiedSecret();
  try {
    await api.lock();
  } finally {
    window.location.reload();
  }
}
