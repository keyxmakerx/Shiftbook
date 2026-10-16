import { describe, expect, it } from 'vitest';
import { confirmsDelete } from './deleteData';

describe('confirmsDelete', () => {
  it('accepts only the word, in any case', () => {
    expect(confirmsDelete('DELETE')).toBe(true);
    expect(confirmsDelete(' delete ')).toBe(true);
    expect(confirmsDelete('')).toBe(false);
    expect(confirmsDelete('DELET')).toBe(false);
    expect(confirmsDelete('delete everything')).toBe(false);
  });
});
