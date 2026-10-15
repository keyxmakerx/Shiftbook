// The paper context menu (owner's ask): right-click an object and a paper sheet pulls out
// from underneath it while the object lifts slightly. Icon rails run along the object's
// side and top; the sheet holds the full labelled options. Click elsewhere or Esc folds it
// back. `PaperMenu.svelte` draws it; components attach it with `use:paperMenu`.

import type { IconName } from './icons';

export interface MenuItem {
  label: string;
  icon: IconName;
  act: () => void;
  /** Also show as an icon button on the side rail. */
  side?: boolean;
  /** Also show as an icon button on the top rail. */
  top?: boolean;
  danger?: boolean;
  /** A dot of this colour before the label (projects). */
  dot?: string | null;
  /** Marks the current choice. */
  checked?: boolean;
  /** Keyboard hint shown on the right. */
  hint?: string;
}

export interface MenuChoice {
  label: string;
  color: string | null;
  checked: boolean;
  act: () => void;
}

export interface MenuSection {
  head?: string;
  items?: MenuItem[];
  /** Compact pills, e.g. which project an entry belongs to. */
  choices?: MenuChoice[];
  /** Colour swatches (project colours). */
  swatches?: { current: string | null; pick: (color: string) => void };
}

export interface MenuSpec {
  sections: MenuSection[];
}

export const menuState = $state<{
  spec: MenuSpec | null;
  anchor: HTMLElement | null;
  serial: number;
}>({ spec: null, anchor: null, serial: 0 });

let closer: ((immediate: boolean) => void) | null = null;

/** PaperMenu registers how to close itself (it plays the fold-back first). */
export function registerCloser(fn: ((immediate: boolean) => void) | null): void {
  closer = fn;
}

export function openMenu(anchor: HTMLElement, spec: MenuSpec): void {
  if (menuState.spec) closer?.(true);
  menuState.anchor = anchor;
  menuState.spec = spec;
  menuState.serial += 1;
}

export function closeMenu(immediate = false): void {
  if (!menuState.spec) return;
  if (closer) closer(immediate);
  else {
    menuState.spec = null;
    menuState.anchor = null;
  }
}

export function menuOpen(): boolean {
  return menuState.spec !== null;
}

/**
 * `use:paperMenu={() => spec}` — right-click (or the Menu key / Shift+F10 while focused)
 * opens the menu for this element. Returning null leaves the browser alone.
 */
export function paperMenu(node: HTMLElement, getSpec: () => MenuSpec | null) {
  let get = getSpec;
  const open = (event: Event) => {
    const spec = get();
    if (!spec) return;
    event.preventDefault();
    event.stopPropagation();
    openMenu(node, spec);
  };
  const onKey = (event: KeyboardEvent) => {
    if (event.target !== node) return;
    if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) open(event);
  };
  node.addEventListener('contextmenu', open);
  node.addEventListener('keydown', onKey);
  return {
    update(next: () => MenuSpec | null) {
      get = next;
    },
    destroy() {
      node.removeEventListener('contextmenu', open);
      node.removeEventListener('keydown', onKey);
    },
  };
}
