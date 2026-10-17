import { describe, expect, it } from 'vitest';
import { backupLabel, fileSize, holds, madeAt } from './backups';

describe('backup words', () => {
  it('names each kind', () => {
    expect(backupLabel('automatic')).toBe('Automatic backup');
    expect(backupLabel('before_restore')).toBe('Saved before a restore');
  });

  it('shows sizes simply', () => {
    expect(fileSize(900)).toBe('900 bytes');
    expect(fileSize(820 * 1024)).toBe('820 KB');
    expect(fileSize(3.4 * 1024 * 1024)).toBe('3.4 MB');
  });

  it('counts what a backup holds', () => {
    const at = '2026-10-16T09:12:00Z';
    expect(holds({ createdAt: at, entries: 42, tickets: 1, projects: 3 })).toBe(
      '42 entries, 1 ticket and 3 projects',
    );
    expect(holds({ createdAt: at, entries: 1, tickets: 0, projects: 1 })).toBe(
      '1 entry, 0 tickets and 1 project',
    );
  });

  it('keeps a time it cannot read as it is', () => {
    expect(madeAt('not a time')).toBe('not a time');
    expect(madeAt('2026-10-16T09:12:00Z', 'en-GB')).toContain('2026');
  });
});
