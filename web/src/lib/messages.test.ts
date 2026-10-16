import { describe, expect, it } from 'vitest';
import { ApiError, toApiError } from './api/DataApi';
import { errorMessage } from './messages';

describe('API errors', () => {
  it('keeps known codes and hides everything else', () => {
    expect(toApiError('unlock_failed').code).toBe('unlock_failed');
    expect(toApiError('busy').code).toBe('busy');
    expect(toApiError('backup_needs_secret').code).toBe('backup_needs_secret');
    expect(toApiError('some long internal detail').code).toBe('unknown');
    expect(toApiError(new Error('boom')).code).toBe('unknown');
    expect(toApiError(undefined).code).toBe('unknown');
    const existing = new ApiError('locked');
    expect(toApiError(existing)).toBe(existing);
  });

  it('gives one generic message for every unlock failure', () => {
    expect(errorMessage('unlock_failed')).toBe(
      "Couldn't unlock. Check your password or recovery key and try again.",
    );
    expect(errorMessage({ secret: 'x' })).toBe('Something went wrong. Please try again.');
  });
});
