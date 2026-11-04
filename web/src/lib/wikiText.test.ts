import { describe, expect, it } from 'vitest';
import { blocks, parts } from './wikiText';

describe('wiki text', () => {
  it('finds links, blanks and code in a line', () => {
    expect(parts('See [[VPN fixes]] for {{site}}, run `show switch` [x] [[]]')).toEqual([
      { kind: 'text', text: 'See ' },
      { kind: 'link', title: 'VPN fixes' },
      { kind: 'text', text: ' for ' },
      { kind: 'blank', name: 'site' },
      { kind: 'text', text: ', run ' },
      { kind: 'code', text: 'show switch' },
      { kind: 'text', text: ' [x] [[]]' },
    ]);
    expect(parts('[[ unclosed')).toEqual([{ kind: 'text', text: '[[ unclosed' }]);
  });

  it('reads headings, lists, code and gaps', () => {
    const b = blocks('# Swap\n\n1. Power off\n  - keep cables\n\n\n```\nshow switch\n```\nDone.\n\n');
    expect(b.map((x) => x.kind)).toEqual(['heading', 'gap', 'item', 'item', 'gap', 'code', 'para']);
    expect(b[2]).toMatchObject({ marker: '1.', depth: 0 });
    expect(b[3]).toMatchObject({ marker: '•', depth: 1 });
    expect(b[5]).toEqual({ kind: 'code', text: 'show switch' });
  });

  it('keeps an unclosed code block', () => {
    expect(blocks('```\nraw')).toEqual([{ kind: 'code', text: 'raw' }]);
  });
});

describe('wiki links in a notepad', () => {
  it('lists each linked page once', async () => {
    const { wikiLinks } = await import('./wikiText');
    expect(wikiLinks('Fixed, see [[VPN fixes]]\nand [[vpn FIXES]] or [[Printers]]')).toEqual(['VPN fixes', 'Printers']);
    expect(wikiLinks(null)).toEqual([]);
  });
});
