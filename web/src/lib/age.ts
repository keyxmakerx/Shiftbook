// How long a still-open ticket has been open (round 21). The days and the amber come from the
// app's core (`shiftbook_core::tray::open_ages`); this is only the words.

import type { OpenAge } from './api/DataApi';

/** The ages by ticket number, lower case, for looking one up. */
export function agesByTicket(ages: OpenAge[]): Record<string, OpenAge> {
  return Object.fromEntries(ages.map((a) => [a.ticket.toLowerCase(), a]));
}

/** A ticket's age, if it is still open. */
export function ageOf(ages: Record<string, OpenAge>, ticket: string): OpenAge | undefined {
  return ages[ticket.toLowerCase()];
}

/** "open 6 days", or '' for a ticket opened today (or not open). */
export function ageWords(age: OpenAge | undefined): string {
  if (!age || age.days < 1) return '';
  return age.days === 1 ? 'open 1 day' : `open ${age.days} days`;
}
