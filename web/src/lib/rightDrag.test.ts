import { describe, expect, it } from 'vitest';
import { AFTER_DRAG_MS, RightPress } from './rightDrag';

describe('right-button drag', () => {
  it('a right-click that opens its menu on press (macOS, Linux) still opens it on release', () => {
    const p = new RightPress();
    p.down(100, 100);
    expect(p.menu('menu', 10)).toBe(true);
    p.move(102, 101);
    expect(p.up(20)).toBe('menu');
  });

  it('a right press that moves is a drag, and its menu never opens', () => {
    const p = new RightPress();
    p.down(100, 100);
    expect(p.menu('menu', 10)).toBe(true);
    p.move(130, 100);
    expect(p.up(20)).toBeNull();
  });

  it('a menu sent on release (Windows) opens after a click and not after a drag', () => {
    const click = new RightPress();
    click.down(0, 0);
    expect(click.up(10)).toBeNull();
    expect(click.menu('menu', 12)).toBe(false);

    const drag = new RightPress();
    drag.down(0, 0);
    drag.move(0, 40);
    expect(drag.up(10)).toBeNull();
    expect(drag.menu('menu', 12)).toBe(true);
    // Later right-clicks, or the Menu key, open as usual.
    expect(drag.menu('menu', 10 + AFTER_DRAG_MS)).toBe(false);
  });

  it('a cancelled press lets the next menu through', () => {
    const p = new RightPress();
    p.down(0, 0);
    p.cancel();
    expect(p.menu('menu', 5)).toBe(false);
  });
});
