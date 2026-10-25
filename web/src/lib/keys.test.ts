import { describe, expect, it } from 'vitest';
import { nextTextSize, shortcutFor, shortcutList, startsHold, viewsFor, type KeyInfo } from './keys';

function key(k: string, extra: Partial<KeyInfo> = {}): KeyInfo {
  return { key: k, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, typing: false, ...extra };
}

describe('keyboard shortcuts', () => {
  it('maps the documented keys', () => {
    expect(shortcutFor(key(' ', { code: 'Space', ctrlKey: true }))).toEqual({ kind: 'capture' });
    expect(shortcutFor(key(' ', { code: 'Space', metaKey: true, typing: true }))).toEqual({ kind: 'capture' });
    expect(shortcutFor(key('l', { ctrlKey: true }))).toEqual({ kind: 'lock' });
    expect(shortcutFor(key('L', { ctrlKey: true, typing: true }))).toEqual({ kind: 'lock' });
    expect(shortcutFor(key('n'))).toEqual({ kind: 'focus-log' });
    expect(shortcutFor(key('1'))).toEqual({ kind: 'view', view: 'desk' });
    expect(shortcutFor(key('2'))).toEqual({ kind: 'view', view: 'day' });
    expect(shortcutFor(key('3'))).toEqual({ kind: 'view', view: 'timesheet' });
    expect(shortcutFor(key('4'))).toEqual({ kind: 'view', view: 'notes' });
    expect(shortcutFor(key('5'))).toEqual({ kind: 'view', view: 'repeating' });
    expect(shortcutFor(key('6'))).toEqual({ kind: 'view', view: 'lists' });
    expect(shortcutFor(key('7'))).toEqual({ kind: 'view', view: 'settings' });
    expect(shortcutFor(key('Escape', { typing: true }))).toEqual({ kind: 'escape' });
    expect(shortcutFor(key('F10', { shiftKey: true }))).toEqual({ kind: 'menu-key' });
    expect(shortcutFor(key('ContextMenu'))).toEqual({ kind: 'menu-key' });
  });

  it('leaves typing alone', () => {
    expect(shortcutFor(key('n', { typing: true }))).toBeNull();
    expect(shortcutFor(key('2', { typing: true }))).toBeNull();
    expect(shortcutFor(key('8'))).toBeNull();
    expect(shortcutFor(key('0'))).toBeNull();
    expect(shortcutFor(key('n', { ctrlKey: true }))).toBeNull();
    expect(shortcutFor(key('l', { ctrlKey: true, altKey: true }))).toBeNull();
    expect(shortcutFor(key('x'))).toBeNull();
  });
});

describe('views by mode', () => {
  it('drops the Timesheet at home and renumbers', () => {
    const home = viewsFor(false);
    expect(home).not.toContain('timesheet');
    expect(shortcutFor(key('3'), home)).toEqual({ kind: 'view', view: 'notes' });
    expect(shortcutFor(key('6'), home)).toEqual({ kind: 'view', view: 'settings' });
    expect(shortcutFor(key('7'), home)).toBeNull();
    expect(viewsFor(true)).toContain('timesheet');
    expect(shortcutList(home)[3]).toEqual({ keys: '1  2  3  4  5  6', what: 'Desk, Day, Notes, Repeating, Lists, Settings' });
  });
});

describe('startsHold', () => {
  it('starts on Ctrl or Meta alone', () => {
    expect(startsHold({ key: 'Control', altKey: false, shiftKey: false })).toBe(true);
    expect(startsHold({ key: 'Meta', altKey: false, shiftKey: false })).toBe(true);
  });
  it('not with other modifiers or keys', () => {
    expect(startsHold({ key: 'Control', altKey: false, shiftKey: true })).toBe(false);
    expect(startsHold({ key: 'l', altKey: false, shiftKey: false })).toBe(false);
  });
});

describe('text size', () => {
  it('Ctrl + − 0 change it, even while typing', () => {
    expect(shortcutFor(key('=', { ctrlKey: true, typing: true }))).toEqual({ kind: 'zoom', step: 1 });
    expect(shortcutFor(key('+', { metaKey: true }))).toEqual({ kind: 'zoom', step: 1 });
    expect(shortcutFor(key('-', { ctrlKey: true }))).toEqual({ kind: 'zoom', step: -1 });
    expect(shortcutFor(key('0', { ctrlKey: true }))).toEqual({ kind: 'zoom', step: 0 });
    expect(shortcutFor(key('0'))).toBeNull();
  });
  it('steps through the sizes and stops at the ends', () => {
    expect(nextTextSize(100, 1)).toBe(115);
    expect(nextTextSize(130, 1)).toBe(130);
    expect(nextTextSize(100, -1)).toBe(90);
    expect(nextTextSize(90, -1)).toBe(90);
    expect(nextTextSize(115, 0)).toBe(100);
    expect(nextTextSize(105, 1)).toBe(130);
  });
});
