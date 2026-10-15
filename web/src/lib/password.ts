// A simple, local password strength estimate for the setup and change-password forms.
// It is a hint for the user, not a rule: the only rule is the minimum length, which the
// app enforces in Rust as well. Nothing here leaves the page.

/** Shortest allowed master password (matches MIN_PASSWORD_CHARS in crates/core). */
export const MIN_PASSWORD_LENGTH = 12;

export type StrengthLevel = 0 | 1 | 2 | 3 | 4;

export interface Strength {
  /** 0 too short, 1 weak, 2 fair, 3 good, 4 strong. */
  level: StrengthLevel;
  /** One or two words for the meter. */
  label: string;
  /** A plain tip, or an empty string when there is nothing to add. */
  hint: string;
}

const LABELS = ['Too short', 'Weak', 'Fair', 'Good', 'Strong'] as const;

// A few passwords and words that attackers try first. Kept short on purpose.
const COMMON = [
  'password',
  'passw0rd',
  'qwerty',
  'letmein',
  'welcome',
  'admin',
  'iloveyou',
  'monkey',
  'dragon',
  'shiftbook',
  'abc123',
  '123456',
];

const SEQUENCES = ['abcdefghijklmnopqrstuvwxyz', '0123456789', 'qwertyuiop', 'asdfghjkl', 'zxcvbnm'];

/** Count characters, not UTF-16 units, so "é" or an emoji counts once. */
export function charLength(text: string): number {
  return Array.from(text).length;
}

function poolSize(text: string): number {
  let size = 0;
  if (/[a-z]/.test(text)) size += 26;
  if (/[A-Z]/.test(text)) size += 26;
  if (/[0-9]/.test(text)) size += 10;
  if (/[ ]/.test(text)) size += 1;
  if (/[^a-zA-Z0-9 ]/.test(text)) size += 33;
  return Math.max(size, 1);
}

/** Characters that just repeat or continue a run ("aaaa", "1234", "qwer") add little. */
function effectiveLength(text: string): number {
  const chars = Array.from(text.toLowerCase());
  let effective = chars.length > 0 ? 1 : 0;
  for (let i = 1; i < chars.length; i++) {
    const prev = chars[i - 1] ?? '';
    const cur = chars[i] ?? '';
    const isRepeat = cur === prev;
    const isRun = SEQUENCES.some((seq) => {
      const at = seq.indexOf(prev);
      return at >= 0 && (seq[at + 1] === cur || (at > 0 && seq[at - 1] === cur));
    });
    effective += isRepeat || isRun ? 0.25 : 1;
  }
  return effective;
}

/** Estimate strength. Long passphrases of plain words score well, which is intended. */
export function passwordStrength(password: string): Strength {
  const length = charLength(password);
  if (length < MIN_PASSWORD_LENGTH) {
    const missing = MIN_PASSWORD_LENGTH - length;
    return {
      level: 0,
      label: LABELS[0],
      hint: `Add ${missing} more character${missing === 1 ? '' : 's'}.`,
    };
  }
  const lower = password.toLowerCase();
  let bits = effectiveLength(password) * Math.log2(poolSize(password));
  const commonHit = COMMON.some((word) => lower.includes(word));
  if (commonHit) bits -= 20;
  const unique = new Set(Array.from(lower)).size;
  if (unique <= 3) bits = Math.min(bits, 20);

  let level: StrengthLevel;
  if (bits < 45) level = 1;
  else if (bits < 60) level = 2;
  else if (bits < 80) level = 3;
  else level = 4;

  let hint = '';
  if (commonHit) hint = 'Avoid common passwords and the app name.';
  else if (level < 3) hint = 'Longer is better: try a few unrelated words with spaces.';
  return { level, label: LABELS[level], hint };
}
