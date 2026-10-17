// Keeps Tab inside an open panel (capture, the details drawer): Tab on the last control
// goes to the first, Shift+Tab on the first goes to the last.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Which element Tab should move to, or null to let the browser carry on. Pure for tests. */
export function wrapTarget<T>(list: readonly T[], current: T | null, back: boolean): T | null {
  if (list.length === 0) return null;
  const i = current === null ? -1 : list.indexOf(current);
  if (back && i <= 0) return list[list.length - 1] ?? null;
  if (!back && (i === -1 || i === list.length - 1)) return list[0] ?? null;
  return null;
}

export function trapTab(event: KeyboardEvent, box: HTMLElement | null | undefined): void {
  if (event.key !== 'Tab' || !box) return;
  const list = [...box.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
  const next = wrapTarget(list, document.activeElement as HTMLElement | null, event.shiftKey);
  if (next) {
    event.preventDefault();
    next.focus();
  }
}
