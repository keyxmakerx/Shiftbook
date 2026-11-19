import { describe, expect, it } from 'vitest';
import type { CheckLine, MopRun } from './api';
import { runLog, stepsLeft, took, type LogWords } from './mopLog';

// Plain UTC words, so the test reads the same on any computer.
const words: LogWords = {
  clock: (ts) => ts.slice(11, 16),
  day: (ts) => ts.slice(0, 10),
};

const check = (key: string, ticked = false, crossed = ticked): CheckLine => ({ key, ticked, crossed });

function run(over: Partial<MopRun> = {}): MopRun {
  return {
    mopId: 'm',
    title: 'Core switch swap',
    ticket: 'CHG0012345',
    templateId: null,
    status: 'done',
    windowOn: null,
    windowAt: null,
    tickTimes: [{ key: 'a', at: '2026-11-18T22:05:00Z' }],
    startedAt: '2026-11-18T22:04:00Z',
    finishedAt: '2026-11-18T23:31:00Z',
    ranMinutes: 90,
    seen: [{ key: 'a', text: 'Gi0/1  up  up\nGi0/2  up  up' }],
    ...over,
  };
}

describe('stepsLeft', () => {
  it('leaves out blank lines, ticked steps and steps crossed out under a ticked heading', () => {
    expect(stepsLeft([check(''), check('h', true), check('s1', false, true), check('s2'), check('s3')])).toBe(2);
    expect(stepsLeft([])).toBe(0);
  });
});

describe('took', () => {
  it('says how long in hours and minutes', () => {
    expect(took('2026-11-18T22:04:00Z', '2026-11-18T23:31:00Z')).toBe('1h 27m');
    expect(took('2026-11-18T22:00:00Z', '2026-11-18T22:40:00Z')).toBe('40m');
    expect(took('2026-11-18T22:00:00Z', '2026-11-19T00:00:00Z')).toBe('2h');
    expect(took(null, '2026-11-18T22:40:00Z')).toBe('');
    expect(took('2026-11-18T22:40:00Z', '2026-11-18T22:00:00Z')).toBe('');
  });
});

describe('runLog', () => {
  const lines = ['# Pre-checks', '1. Check the uplinks', '', '2. Swap the switch'];
  const checks = [check(''), check('a', true), check(''), check('b')];

  it('carries the day, the start, the finish and how long it took', () => {
    const log = runLog(run(), 'Core switch swap', 'Done', lines, checks, words).split('\n');
    expect(log[0]).toBe('CHG0012345 · Core switch swap');
    expect(log[1]).toBe('Done · 2026-11-18 · started 22:04 · finished 23:31 · took 1h 27m');
    expect(log[2]).toBe('');
  });

  it('puts what was seen under its step, columns kept', () => {
    const log = runLog(run(), 'Core switch swap', 'Done', lines, checks, words).split('\n');
    expect(log.slice(3)).toEqual([
      '      ☐ # Pre-checks',
      '22:05 ✓ 1. Check the uplinks',
      '          Gi0/1  up  up',
      '          Gi0/2  up  up',
      '      ☐ 2. Swap the switch',
    ]);
  });

  it('says which day a run over midnight finished', () => {
    const log = runLog(run({ finishedAt: '2026-11-19T00:31:00Z' }), 't', 'Done', lines, checks, words);
    expect(log.split('\n')[1]).toBe('Done · 2026-11-18 · started 22:04 · finished 2026-11-19 00:31 · took 2h 27m');
  });

  it('still has a day when the run never went Running', () => {
    const log = runLog(run({ startedAt: null, ranMinutes: null }), 't', 'Done', lines, checks, words);
    expect(log.split('\n')[1]).toBe('Done · 2026-11-18 · finished 23:31');
    const bare = runLog(run({ startedAt: null, finishedAt: null, tickTimes: [], seen: [] }), 't', 'Ready', lines, [], words);
    expect(bare.split('\n')[1]).toBe('Ready');
  });
});
