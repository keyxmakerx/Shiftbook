import { describe, expect, it } from 'vitest';
import { hasHome, hasWork, wordsFor } from './words';

describe('words by mode', () => {
  it('keeps work words for Work and Both', () => {
    expect(wordsFor('work').tickets).toBe('Tickets');
    expect(wordsFor('both').meetings).toBe('Meetings');
    expect(wordsFor(undefined).logWork).toBe('Log work');
  });

  it('uses everyday words at home', () => {
    const w = wordsFor('home');
    expect(w.tickets).toBe('References');
    expect(w.ticket).toBe('reference');
    expect(w.meetings).toBe('Regular events');
    expect(w.mop).toBe('Step-by-step guide');
  });

  it('says what shows where', () => {
    expect(hasWork('work') && !hasHome('work')).toBe(true);
    expect(!hasWork('home') && hasHome('home')).toBe(true);
    expect(hasWork('both') && hasHome('both')).toBe(true);
  });
});
