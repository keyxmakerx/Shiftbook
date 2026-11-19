import { describe, expect, it } from 'vitest';
import { daysSummary, helperDefaults, helperPrefs } from './helpers';

describe('the few helpers screen', () => {
  it('starts with the wrap-up on and Monday to Friday', () => {
    const d = helperDefaults();
    expect(d.wrapup).toBe(true);
    expect(d.timer).toBe(false);
    expect(daysSummary(d.days)).toBe('Mon–Fri');
  });

  it('saves each tick as an ordinary setting', () => {
    expect(helperPrefs(helperDefaults(), 'work')).toEqual([
      ['day.wrapup_at', '17:00'],
      ['day.timer', 'false'],
      ['timesheet.use', 'false'],
      ['day.targets', '480,480,480,480,480,0,0'],
    ]);
    const mine = { ...helperDefaults(), wrapup: false, timer: true, timesheet: true, days: [true, false, true, false, true, true, false] };
    expect(helperPrefs(mine, 'both')).toEqual([
      ['day.wrapup_at', ''],
      ['day.timer', 'true'],
      ['timesheet.use', 'true'],
      ['day.targets', '480,0,480,0,480,480,0'],
    ]);
  });

  it('leaves the timesheet and the hours alone at home', () => {
    expect(helperPrefs(helperDefaults(), 'home').map(([k]) => k)).toEqual(['day.wrapup_at', 'day.timer']);
  });

  it('turns the wrap-up off when its time is not a time', () => {
    expect(helperPrefs({ ...helperDefaults(), wrapupAt: '' }, 'home')[0]).toEqual(['day.wrapup_at', '']);
  });

  it('says the days in a few words', () => {
    expect(daysSummary([true, false, true, false, true, false, false])).toBe('Mon, Wed, Fri');
    expect(daysSummary([false, false, false, false, false, true, true])).toBe('Sat, Sun');
    expect(daysSummary(Array(7).fill(false))).toBe('No days');
    expect(daysSummary(Array(7).fill(true))).toBe('Mon–Sun');
  });
});
