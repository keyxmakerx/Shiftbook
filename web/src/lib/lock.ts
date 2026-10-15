import { api } from './api';
import { clearCopiedSecret } from './clipboard';

/**
 * Lock now: empty the clipboard if we copied a secret, ask the app to close the data
 * file and forget the key, then reload the page so nothing decrypted stays in memory.
 * The page reloads even if the lock call fails; the unlock screen shows the real state.
 */
export async function lockAndReload(): Promise<void> {
  await clearCopiedSecret();
  try {
    await api.lock();
  } finally {
    window.location.reload();
  }
}
