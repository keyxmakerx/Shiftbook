// Small focus helpers shared by components.

/** Svelte action: focus the element as soon as it appears (after the click that opened it). */
export function focusNow(node: HTMLElement): void {
  requestAnimationFrame(() => {
    node.focus();
    if (node instanceof HTMLInputElement) node.select();
  });
}
