import { describe, expect, it } from 'vitest';
import { sameWork } from './sameWork';

const e = (id: string, isTray = false) => ({ id, isTray });

describe('sameWork', () => {
  it('is the shown entry plus the same work folded into its card', () => {
    // A ticket worked on Friday and again today shows as one card; moving that card to a
    // project must move both, or the card splits and Friday's looks like a new copy.
    expect(sameWork(e('today'), [e('friday'), e('monday')]).map((x) => x.id)).toEqual(['today', 'friday', 'monday']);
  });

  it('is just the entry when nothing is folded in', () => {
    expect(sameWork(e('only')).map((x) => x.id)).toEqual(['only']);
  });

  it('leaves out tray roll-ups and the shown entry repeated', () => {
    expect(sameWork(e('a'), [e('a'), e('tray', true), e('b')]).map((x) => x.id)).toEqual(['a', 'b']);
  });
});
