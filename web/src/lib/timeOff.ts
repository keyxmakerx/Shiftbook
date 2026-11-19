// Booking time off (round 21): the words for a plan. Which days get booked, and for how
// long, comes from the app's core (`shiftbook_core::timeoff::plan_time_off`).

import type { TimeOffPlan } from './api/DataApi';
import { dayOfMonth, weekdayShort } from './format';

/** "5 work days, 40 h", "1 work day, 8 h", or '' when nothing gets booked. */
export function offSummary(plan: TimeOffPlan): string {
  if (plan.booked < 1) return '';
  return `${plan.booked} work ${plan.booked === 1 ? 'day' : 'days'}, ${plan.hours} h`;
}

/** What is skipped, in a sentence, or '' when every day is booked. */
export function offSkipped(plan: TimeOffPlan): string {
  const free = plan.days.filter((d) => d.skip === 'free').length;
  const logged = plan.days.filter((d) => d.skip === 'logged').length;
  if (free + logged === 0) return '';
  if (plan.booked < 1) {
    if (logged === 0) return 'Nothing to book: these are days you don\'t usually work.';
    if (free === 0) return 'Nothing to book: every one of these days has something logged already.';
    return 'Nothing to book: these days are either days you don\'t usually work or already have something logged.';
  }
  const parts: string[] = [];
  if (free > 0) parts.push(free === 1 ? '1 day you don\'t usually work' : `${free} days you don't usually work`);
  if (logged > 0) parts.push(logged === 1 ? '1 day you already logged' : `${logged} days you already logged`);
  return `Skipped: ${parts.join(' and ')}.`;
}

/** "Book 5 days", "Book 1 day". */
export function offButton(plan: TimeOffPlan | null): string {
  if (!plan || plan.booked < 1) return 'Book';
  return `Book ${plan.booked} ${plan.booked === 1 ? 'day' : 'days'}`;
}

/** What the toast says after booking: "Mon 2 booked off" or "5 days booked off (40 h)". */
export function offDone(dates: string[], hours: string): string {
  if (dates.length === 1) return `${weekdayShort(dates[0])} ${dayOfMonth(dates[0])} booked off`;
  return `${dates.length} days booked off (${hours} h)`;
}
