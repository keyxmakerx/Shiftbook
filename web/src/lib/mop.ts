// Display helpers for the MOP notebook (round 16): the colours Numbered mode paints, the
// counts in the status bar, Find, and reading dropped files. The rules (blanks, ticks and
// what a tick crosses out, file text and titles) live in crates/core/src/mop.rs.
import type { MopBook, MopFile } from './api';

/** How a line is coloured: heading, rollback heading, numbered step, check line or plain. */
export type LineTone = 'h' | 'rb' | 'n' | 'v' | '';

/** A piece of a painted line. */
export interface Piece {
  text: string;
  tone: LineTone | 'blank' | 'num';
}

const HEADING = /^\s*#{1,6}\s/;
const ROLLBACK = /\b(rollback|roll back|backout|back out|revert)\b/i;
const STEP = /^(\s*(?:step\s+\d+|\d{1,4}(?:\.\d{1,4})*[.):]?))(?=\s|$)/i;
const CHECK = /^\s*(verify|expected|expect|check|confirm|result)\b/i;
const BLANK = /\[\[[^[\]\n]{1,60}\]\]/g;

/** Colour every line of a MOP. A heading about rollback turns the steps under it red. */
export function paint(text: string): Piece[][] {
  let rollback = false;
  return text.split('\n').map((line) => {
    if (HEADING.test(line)) {
      rollback = ROLLBACK.test(line);
      return blanks(line, rollback ? 'rb' : 'h');
    }
    const step = STEP.exec(line);
    if (step && !/^\s*\d{1,4}\s/.test(line)) {
      return [{ text: step[1]!, tone: rollback ? 'rb' : 'num' }, ...blanks(line.slice(step[1]!.length), '')];
    }
    return blanks(line, CHECK.test(line) ? 'v' : '');
  });
}

/** Split a line around its `[[blanks]]`. */
function blanks(line: string, tone: LineTone): Piece[] {
  const out: Piece[] = [];
  let at = 0;
  for (const m of line.matchAll(BLANK)) {
    if (m[0].slice(2, -2).trim() === '') continue;
    if (m.index > at) out.push({ text: line.slice(at, m.index), tone });
    out.push({ text: m[0], tone: 'blank' });
    at = m.index + m[0].length;
  }
  if (at < line.length || out.length === 0) out.push({ text: line.slice(at), tone });
  return out;
}

/** What the status bar shows. */
export interface Counts {
  lines: number;
  chars: number;
  /** Line and column of the cursor, from 1. */
  line: number;
  col: number;
  selChars: number;
  selLines: number;
}

/** Count lines and characters, and what's selected between `start` and `end`. */
export function counts(text: string, start: number, end: number): Counts {
  const before = text.slice(0, start);
  const lineStart = before.lastIndexOf('\n') + 1;
  const sel = text.slice(Math.min(start, end), Math.max(start, end));
  return {
    lines: text.split('\n').length,
    chars: [...text].length,
    line: before.split('\n').length,
    col: [...text.slice(lineStart, start)].length + 1,
    selChars: [...sel].length,
    selLines: sel ? sel.split('\n').length : 0,
  };
}

/** Where `query` appears in `text`, ignoring case: start offsets. */
export function findAll(text: string, query: string): number[] {
  const q = query.toLowerCase();
  if (!q) return [];
  const hay = text.toLowerCase();
  const out: number[] = [];
  for (let i = hay.indexOf(q); i >= 0 && out.length < 10_000; i = hay.indexOf(q, i + q.length)) out.push(i);
  return out;
}

/** The stored form of the notebook pref (checked again by `MopBook::parse` in core). */
export function formatBook(b: MopBook): string {
  const r = Math.round;
  const tabs = b.tabs.length ? b.tabs.join(',') : '-';
  return `${b.shown ? 1 : 0} ${b.mode} ${r(b.x)} ${r(b.y)} ${r(b.w)} ${r(b.h)} ${b.view} ${b.numbers ? 1 : 0} ${b.active ?? '-'} ${tabs}`;
}

/** Files that look like they are meant to be text. Others are still tried; core says why not. */
const TEXT_NAME = /\.(txt|text|md|markdown|log|mop|cfg|conf|ini)$/i;

/** True when a file is probably a text MOP (the import list ticks these by default). */
export function looksLikeText(name: string): boolean {
  return TEXT_NAME.test(name) || !name.includes('.');
}

/** Read dropped or picked files as text, in name order, at most `max`. */
export async function readFiles(files: Iterable<File>, max = 50): Promise<MopFile[]> {
  const list = [...files].slice(0, max).sort((a, b) => a.name.localeCompare(b.name));
  return Promise.all(
    list.map(async (f) => ({
      name: f.name,
      // Files over a few MB can't be one note anyway; core says so.
      text: f.size > 2_000_000 ? '\0' : await f.text(),
      template: /template/i.test(f.name),
    })),
  );
}

/** True while something being dragged over the window holds files. */
export function hasFiles(e: DragEvent): boolean {
  return [...(e.dataTransfer?.types ?? [])].includes('Files');
}
