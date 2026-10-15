import { describe, expect, it } from 'vitest';
import { groupMatches, normalizeGroup, pickCheckGroups, recoveryGroups } from './recoveryKey';

const KEY = 'AB12-CD34-EF56-GH78-JK90-MN01-PQ23-RS45-TV67-WX89-YZ01-0123-4560';

describe('recoveryGroups', () => {
  it('splits the key into its 13 groups', () => {
    const groups = recoveryGroups(KEY);
    expect(groups).toHaveLength(13);
    expect(groups[0]).toBe('AB12');
    expect(groups[12]).toBe('4560');
  });
});

describe('groupMatches', () => {
  it('ignores case, spaces and hyphens', () => {
    expect(groupMatches('AB12', 'ab12')).toBe(true);
    expect(groupMatches('AB12', ' a b-1 2 ')).toBe(true);
  });

  it('reads O as 0 and I or L as 1, like the app', () => {
    expect(normalizeGroup('oIl0')).toBe('0110');
    expect(groupMatches('MN01', 'mnOl')).toBe(true);
    expect(groupMatches('0123', 'OI23')).toBe(true);
  });

  it('rejects wrong or empty groups', () => {
    expect(groupMatches('AB12', 'AB13')).toBe(false);
    expect(groupMatches('AB12', 'AB1')).toBe(false);
    expect(groupMatches('AB12', '')).toBe(false);
    expect(groupMatches('', '')).toBe(false);
  });
});

describe('pickCheckGroups', () => {
  it('picks distinct positions in order', () => {
    const values = [0.99, 0.99, 0.1];
    let i = 0;
    const picked = pickCheckGroups(13, 2, () => values[i++] ?? 0);
    expect(picked).toEqual([1, 12]);
  });

  it('uses real randomness by default and stays in range', () => {
    for (let run = 0; run < 50; run++) {
      const picked = pickCheckGroups(13, 2);
      expect(picked).toHaveLength(2);
      expect(new Set(picked).size).toBe(2);
      for (const p of picked) {
        expect(p).toBeGreaterThanOrEqual(0);
        expect(p).toBeLessThan(13);
      }
    }
  });

  it('never asks for more groups than exist', () => {
    expect(pickCheckGroups(1, 2)).toEqual([0]);
    expect(pickCheckGroups(0, 2)).toEqual([]);
  });
});
