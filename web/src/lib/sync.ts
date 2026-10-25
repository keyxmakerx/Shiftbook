// Plain words for the "Bring in changes?" check (round 15). The merge itself, and every count,
// comes from the app (crates/storage/src/sync.rs).
import type { SyncReport } from './api/DataApi';

export interface ReportLine {
  label: string;
  value: string;
}

function both(added: number, changed: number): string {
  const parts = [];
  if (added) parts.push(`${added} new`);
  if (changed) parts.push(`${changed} changed`);
  return parts.join(' · ');
}

/** The lines of the check, leaving out what's zero. */
export function reportLines(r: SyncReport): ReportLine[] {
  const lines: ReportLine[] = [];
  const add = (label: string, n: number, value = String(n)) => {
    if (n > 0) lines.push({ label, value });
  };
  add('New entries', r.newEntries);
  add('Changed entries', r.changedEntries);
  add('Notes', r.newNotes + r.changedNotes, both(r.newNotes, r.changedNotes));
  add('Projects', r.newProjects + r.changedProjects, both(r.newProjects, r.changedProjects));
  add('Tickets, scraps and other things', r.other);
  add('Deleted (can be brought back)', r.deleted);
  add('Newer here, kept as they are', r.keptHere);
  add('Left out (already here in another form)', r.leftOut);
  return lines;
}

/** True when the file holds nothing new for this device. */
export function nothingNew(r: SyncReport): boolean {
  return r.newEntries + r.changedEntries + r.newNotes + r.changedNotes + r.newProjects + r.changedProjects + r.other + r.deleted === 0;
}
