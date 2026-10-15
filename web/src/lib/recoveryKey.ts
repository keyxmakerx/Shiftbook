// Helpers for showing the recovery key and checking that the user copied it down.
// The key itself is checked in Rust; these only drive the "retype two groups" step.

/** Split `ABCD-EFGH-…` into its groups. */
export function recoveryGroups(key: string): string[] {
  return key
    .split('-')
    .map((g) => g.trim())
    .filter((g) => g.length > 0);
}

/**
 * Normalise a typed group the way the app reads recovery keys: ignore case, spaces and
 * hyphens; O reads as 0; I and L read as 1.
 */
export function normalizeGroup(text: string): string {
  return text
    .toUpperCase()
    .replace(/[\s-]/g, '')
    .replace(/O/g, '0')
    .replace(/[IL]/g, '1');
}

/** True when `typed` is the same group as `expected` after normalising both. */
export function groupMatches(expected: string, typed: string): boolean {
  const want = normalizeGroup(expected);
  return want.length > 0 && normalizeGroup(typed) === want;
}

/**
 * Pick `count` different group positions (0-based, ascending) out of `total`.
 * `random` returns a number in [0, 1); it defaults to crypto randomness.
 */
export function pickCheckGroups(
  total: number,
  count: number,
  random: () => number = cryptoRandom,
): number[] {
  const n = Math.max(0, Math.floor(total));
  const want = Math.min(Math.max(0, Math.floor(count)), n);
  const picked = new Set<number>();
  while (picked.size < want) {
    picked.add(Math.min(n - 1, Math.floor(random() * n)));
  }
  return [...picked].sort((a, b) => a - b);
}

function cryptoRandom(): number {
  const buf = new Uint32Array(1);
  globalThis.crypto.getRandomValues(buf);
  return (buf[0] ?? 0) / 2 ** 32;
}
