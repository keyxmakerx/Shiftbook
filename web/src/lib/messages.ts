import { toApiError } from './api/DataApi';
import type { ApiErrorCode } from './api/DataApi';

const MESSAGES: Record<ApiErrorCode, string> = {
  unlock_failed: "Couldn't unlock. Check your password or recovery key and try again.",
  weak_password: 'Use at least 12 characters for your password.',
  busy: 'Still working on the last request. Please wait a moment and try again.',
  already_set_up: 'Shiftbook is already set up. Restart the app to unlock it.',
  not_set_up: "Shiftbook hasn't been set up yet. Restart the app to set it up.",
  locked: 'Shiftbook is locked. Unlock it first.',
  storage_error:
    "Something went wrong reading or saving your data. Try again, and restart Shiftbook if it keeps happening.",
  unknown: 'Something went wrong. Please try again.',
};

/** A plain sentence for any error a DataApi call rejected with. */
export function errorMessage(raw: unknown): string {
  return MESSAGES[toApiError(raw).code];
}
