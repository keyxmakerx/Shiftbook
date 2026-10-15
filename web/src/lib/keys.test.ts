import { describe, expect, it } from 'vitest';
import { shortcutFor, type KeyInfo } from './keys';

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
    expect(shortcutFor(key('2'))).toEqual({ kind: 'view', view: 'timesheet' });
    expect(shortcutFor(key('3'))).toEqual({ kind: 'view', view: 'customize' });
    expect(shortcutFor(key('Escape', { typing: true }))).toEqual({ kind: 'escape' });
    expect(shortcutFor(key('F10', { shiftKey: true }))).toEqual({ kind: 'menu-key' });
    expect(shortcutFor(key('ContextMenu'))).toEqual({ kind: 'menu-key' });
  });

  it('leaves typing alone', () => {
    expect(shortcutFor(key('n', { typing: true }))).toBeNull();
    expect(shortcutFor(key('2', { typing: true }))).toBeNull();
    expect(shortcutFor(key('4'))).toBeNull();
    expect(shortcutFor(key('0'))).toBeNull();
    expect(shortcutFor(key('n', { ctrlKey: true }))).toBeNull();
    expect(shortcutFor(key('l', { ctrlKey: true, altKey: true }))).toBeNull();
    expect(shortcutFor(key('x'))).toBeNull();
  });
});
