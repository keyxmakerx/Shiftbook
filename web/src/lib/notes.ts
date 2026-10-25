// Display helpers for notes: kind names, the match highlight and the list snippet.
// Matching and ranking happen in the app (crates/core/src/search.rs); these only mark
// where the typed words appear so the list can show them.

import type { NoteKind } from './api';
import { dayMonth, relativeDay, toIso, weekdayShort } from './format';

export const KINDS: readonly NoteKind[] = ['jot', 'daily', 'meeting', 'page', 'mop'];

// The MOP kind is called "Step-by-step guide" at home (round 15); the store says which.
let mopWord = (): string => 'MOP';

/** Let the store name the MOP kind for the current "What's Shiftbook for?". */
export function useMopWord(word: () => string): void {
  mopWord = word;
}

export const KIND_LABEL: Record<NoteKind, string> = {
  jot: 'Jot',
  daily: 'Daily',
  meeting: 'Meeting',
  page: 'Page',
  get mop() {
    return mopWord();
  },
};

/** A piece of text, marked when it matches one of the typed words. */
export interface Part {
  text: string;
  hit: boolean;
}

function words(query: string): string[] {
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter((w) => w.length > 0);
}

/** Split `text` into parts, marking every place a word of `query` appears (ignoring case). */
export function highlight(text: string, query: string): Part[] {
  const ws = words(query);
  if (ws.length === 0 || !text) return text ? [{ text, hit: false }] : [];
  const lower = text.toLowerCase();
  const marked = new Array<boolean>(text.length).fill(false);
  for (const w of ws) {
    let at = lower.indexOf(w);
    while (at >= 0) {
      marked.fill(true, at, at + w.length);
      at = lower.indexOf(w, at + w.length);
    }
  }
  const parts: Part[] = [];
  for (let i = 0; i < text.length; i++) {
    const last = parts[parts.length - 1];
    if (last && last.hit === marked[i]) last.text += text[i];
    else parts.push({ text: text[i]!, hit: marked[i]! });
  }
  return parts;
}

/**
 * A one-line taste of the body for the list: around the first match when searching,
 * else the start (skipping a first line that repeats the title).
 */
export function snippet(body: string, title: string, query: string, max = 120): string {
  const flat = body.replace(/\s+/g, ' ').trim();
  const lower = flat.toLowerCase();
  const first = words(query)
    .map((w) => lower.indexOf(w))
    .filter((i) => i >= 0)
    .sort((a, b) => a - b)[0];
  if (first !== undefined && first > max / 3) {
    const start = flat.lastIndexOf(' ', first - Math.floor(max / 3)) + 1;
    const cut = flat.slice(start, start + max);
    return '…' + cut + (start + max < flat.length ? '…' : '');
  }
  const rest = flat.toLowerCase().startsWith(title.toLowerCase()) ? flat.slice(title.length).trim() : flat;
  return rest.length > max ? rest.slice(0, max).trimEnd() + '…' : rest;
}

/** "Moves to the archive in 3 days", or "tomorrow". */
export function archiveWarning(days: number): string {
  return days <= 1 ? 'This note moves to the archive tomorrow' : `This note moves to the archive in ${days} days`;
}

/** When a note last changed, from its UTC timestamp: "Today", "Yesterday", "Fri 2 October", or with the year when it isn't this year. */
export function changedOn(timestamp: string, today: string): string {
  const iso = toIso(new Date(timestamp));
  const near = relativeDay(iso, today);
  if (near === 'Today' || near === 'Yesterday') return near;
  const day = `${weekdayShort(iso)} ${dayMonth(iso)}`;
  return iso.slice(0, 4) === today.slice(0, 4) ? day : `${day} ${iso.slice(0, 4)}`;
}
