import { describe, expect, it } from 'vitest';
import { charLength, MIN_PASSWORD_LENGTH, passwordStrength } from './password';

describe('passwordStrength', () => {
  it('calls anything under 12 characters too short and says how many are missing', () => {
    expect(MIN_PASSWORD_LENGTH).toBe(12);
    expect(passwordStrength('')).toMatchObject({ level: 0, label: 'Too short' });
    expect(passwordStrength('elevenchars').hint).toBe('Add 1 more character.');
    expect(passwordStrength('abc').hint).toBe('Add 9 more characters.');
  });

  it('counts characters, not UTF-16 units', () => {
    expect(charLength('ééé')).toBe(3);
    expect(charLength('😀😀')).toBe(2);
    expect(passwordStrength('😀'.repeat(11)).level).toBe(0);
    expect(passwordStrength('😀'.repeat(12)).level).toBeGreaterThan(0);
  });

  it('rates repeats, runs and common words as weak', () => {
    expect(passwordStrength('aaaaaaaaaaaa').level).toBe(1);
    expect(passwordStrength('123456789012').level).toBe(1);
    expect(passwordStrength('abcdefghijkl').level).toBe(1);
    const common = passwordStrength('password1234');
    expect(common.level).toBe(1);
    expect(common.hint).toMatch(/common/);
  });

  it('rates long passphrases and mixed passwords as strong', () => {
    expect(passwordStrength('purple tractor seven lanterns').level).toBe(4);
    expect(passwordStrength('Tq9#vL2!mZ7@').level).toBeGreaterThanOrEqual(3);
  });

  it('gets stronger as the password gets longer', () => {
    const levels = ['river stone k', 'river stone kite', 'river stone kite moss'].map(
      (p) => passwordStrength(p).level,
    );
    expect(levels[0]).toBeLessThanOrEqual(levels[1] ?? 0);
    expect(levels[1]).toBeLessThanOrEqual(levels[2] ?? 0);
  });
});
