// Words that change with "What's Shiftbook for?" (round 15). Work keeps Shiftbook's words;
// Home swaps the work ones for everyday ones. Both keeps the work words (it still has
// tickets and meetings) and adds the home pieces. Only labels live here; what each mode
// shows and its defaults come from crates/core/src/home.rs.
import type { UseMode } from './api';

export interface Words {
  /** "Tickets" / "References" (the tray and ticket boxes). */
  tickets: string;
  /** "ticket" / "reference", lower case, one of them. */
  ticket: string;
  /** "Meetings" / "Regular events". */
  meetings: string;
  /** "MOP" / "Step-by-step guide" (the note kind). */
  mop: string;
  /** "MOP notebook" / "Guide notebook" (the round 16 window). */
  book: string;
  /** "Log work" / "Done something?". */
  logWork: string;
}

const WORK: Words = {
  tickets: 'Tickets',
  ticket: 'ticket',
  meetings: 'Meetings',
  mop: 'MOP',
  book: 'MOP notebook',
  logWork: 'Log work',
};

const HOME: Words = {
  tickets: 'References',
  ticket: 'reference',
  meetings: 'Regular events',
  mop: 'Step-by-step guide',
  book: 'Guide notebook',
  logWork: 'Done something?',
};

export function wordsFor(mode: UseMode | undefined): Words {
  return mode === 'home' ? HOME : WORK;
}

/** True when the Timesheet and the hours ring show (Work and Both). */
export function hasWork(mode: UseMode | undefined): boolean {
  return mode !== 'home';
}

/** True when the home pieces show: the Done button and time left optional (Home and Both). */
export function hasHome(mode: UseMode | undefined): boolean {
  return mode === 'home' || mode === 'both';
}
