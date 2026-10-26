import { describe, expect, it } from 'vitest';
import { startLine } from './renewal';

describe('startLine', () => {
  it('files a renewal under the thing', () => {
    expect(startLine('www.example.com certificate', true)).toBe(
      'Renew www.example.com certificate #www.example.com-certificate ',
    );
  });

  it('uses the plain name for things that repeat', () => {
    expect(startLine('  Monthly   patching ', false)).toBe('Monthly patching #Monthly-patching ');
  });
});
