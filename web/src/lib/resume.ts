// "Where was I?" helpers for the Desk card. When the card is due and what it offers are
// decided by the app (crates/core/src/resume.rs); this file only notices returns and
// words things.
import type { Entry, ResumeCard } from './api/DataApi';

/** A quiet stretch shorter than this isn't worth asking about. */
export const RETURN_PROBE_MS = 60 * 1000;

/**
 * Called on every keyboard or mouse event. Returns the new last-activity time and, when
 * the user is back after at least RETURN_PROBE_MS, how many whole minutes they were away.
 */
export function noticeReturn(
  lastActivity: number,
  now: number,
): { lastActivity: number; awayMinutes: number | null } {
  const gap = now - lastActivity;
  return { lastActivity: now, awayMinutes: gap >= RETURN_PROBE_MS ? Math.floor(gap / 60000) : null };
}

/** The line Log work gets on Carry on: an entry's tickets and title, or the first thing on the list. */
export function carryOnText(card: ResumeCard): string {
  if (card.entry) return entryText(card.entry);
  return card.list[0]?.text ?? '';
}

function entryText(e: Entry): string {
  const tickets = e.tickets.map((t) => t.number);
  const words = e.description.trim();
  const missing = tickets.filter((t) => !words.includes(t));
  return [...missing, words].filter(Boolean).join(' ');
}

/** The card's title: the first thing on the list, or the entry's title (its tickets if it has none). */
export function cardTitle(card: ResumeCard): string {
  if (card.entry) {
    return card.entry.description.trim() || card.entry.tickets.map((t) => t.number).join(', ');
  }
  return card.list[0]?.text ?? '';
}

/** "You said you'd start with", "Before your break" or "Last time". */
export function cardLead(card: ResumeCard): string {
  if (!card.entry) return "You said you'd start with";
  return card.trigger === 'break' ? 'Before your break' : 'Last time';
}

/** "Last logged 52 minutes ago", "… 3 hours ago", "… yesterday", "… 4 days ago". */
export function lastLogged(changedAt: string | null, now: Date): string {
  if (!changedAt) return '';
  const then = new Date(changedAt);
  const ms = now.getTime() - then.getTime();
  if (Number.isNaN(ms)) return '';
  const minutes = Math.max(0, Math.floor(ms / 60000));
  if (minutes < 1) return 'Last logged just now';
  if (minutes < 60) return `Last logged ${minutes} minute${minutes === 1 ? '' : 's'} ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Last logged ${hours} hour${hours === 1 ? '' : 's'} ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? 'Last logged yesterday' : `Last logged ${days} days ago`;
}
