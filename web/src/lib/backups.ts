// Plain words for the Backups screen. The backups themselves are made and checked by the app.
import type { BackupFile, BackupSummary } from './api/DataApi';

/** "Automatic backup" or "Saved before a restore". */
export function backupLabel(kind: BackupFile['kind']): string {
  return kind === 'automatic' ? 'Automatic backup' : 'Saved before a restore';
}

/** A file size people can read: "820 KB", "3.4 MB". */
export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** When a backup was made, in the user's own time zone: "Thu 16 Oct 2026, 09:12". */
export function madeAt(createdAt: string, locale?: string): string {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return createdAt;
  return date.toLocaleString(locale, {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function count(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "42 entries, 17 tickets and 3 projects". */
export function holds(summary: BackupSummary): string {
  return `${count(summary.entries, 'entry', 'entries')}, ${count(summary.tickets, 'ticket', 'tickets')} and ${count(summary.projects, 'project', 'projects')}`;
}
