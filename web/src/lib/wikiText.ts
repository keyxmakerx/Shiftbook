// How a wiki page reads (round 18): its plain text shown with headings (#, ##, ###), numbered
// and bulleted lines, ``` code blocks, `code`, [[links]] to other pages and {{blanks}} in
// templates. Only display: what links where is worked out in crates/core/src/wiki.rs. Nothing
// here makes HTML; the page builds elements from these parts, so text is always just text.

export type Part =
  | { kind: 'text'; text: string }
  | { kind: 'link'; title: string }
  | { kind: 'blank'; name: string }
  | { kind: 'code'; text: string };

export type Block =
  | { kind: 'heading'; level: 1 | 2 | 3; parts: Part[] }
  | { kind: 'item'; marker: string; depth: number; parts: Part[] }
  | { kind: 'para'; parts: Part[] }
  | { kind: 'code'; text: string }
  | { kind: 'gap' };

const LONGEST_INSIDE = 60;

/** The parts of one line: plain text, [[links]], {{blanks}} and `code`. */
export function parts(line: string): Part[] {
  const out: Part[] = [];
  let text = '';
  let i = 0;
  const flush = () => {
    if (text) out.push({ kind: 'text', text });
    text = '';
  };
  while (i < line.length) {
    const rest = line.slice(i);
    const pair = rest.startsWith('[[') ? ']]' : rest.startsWith('{{') ? '}}' : rest.startsWith('`') ? '`' : null;
    if (pair) {
      const open = pair === '`' ? 1 : 2;
      const close = rest.indexOf(pair, open);
      const inside = close > 0 ? rest.slice(open, close) : '';
      const ok =
        close > 0 &&
        inside.trim() !== '' &&
        (pair === '`' || (inside.trim().length <= LONGEST_INSIDE && !inside.includes(rest[0]!)));
      if (ok) {
        flush();
        if (pair === ']]') out.push({ kind: 'link', title: inside.trim() });
        else if (pair === '}}') out.push({ kind: 'blank', name: inside.trim() });
        else out.push({ kind: 'code', text: inside });
        i += close + pair.length;
        continue;
      }
    }
    text += line[i];
    i += 1;
  }
  flush();
  return out;
}

/** A page's text as blocks to show. */
export function blocks(body: string): Block[] {
  const out: Block[] = [];
  const lines = body.replace(/\r\n?/g, '\n').split('\n');
  let code: string[] | null = null;
  for (const line of lines) {
    if (code) {
      if (line.trim().startsWith('```')) {
        out.push({ kind: 'code', text: code.join('\n') });
        code = null;
      } else code.push(line);
      continue;
    }
    if (line.trim().startsWith('```')) {
      code = [];
      continue;
    }
    if (!line.trim()) {
      if (out.length && out[out.length - 1]!.kind !== 'gap') out.push({ kind: 'gap' });
      continue;
    }
    const heading = /^(#{1,3})\s+(.*)$/.exec(line.trim());
    if (heading) {
      out.push({ kind: 'heading', level: heading[1]!.length as 1 | 2 | 3, parts: parts(heading[2]!) });
      continue;
    }
    const item = /^(\s*)(\d+(?:\.\d+)*[.)]|[-*•])\s+(.*)$/.exec(line);
    if (item) {
      const depth = Math.min(4, Math.floor(item[1]!.replace(/\t/g, '  ').length / 2));
      const marker = /^[-*•]$/.test(item[2]!) ? '•' : item[2]!;
      out.push({ kind: 'item', marker, depth, parts: parts(item[3]!) });
      continue;
    }
    out.push({ kind: 'para', parts: parts(line) });
  }
  if (code) out.push({ kind: 'code', text: code.join('\n') });
  while (out.length && out[out.length - 1]!.kind === 'gap') out.pop();
  return out;
}

/** The pages `text` links to with [[Title]], each once (ignoring case), for link chips under a
 *  notepad. Which notes link to a page is worked out in crates/core/src/wiki.rs. */
export function wikiLinks(text: string | null | undefined): string[] {
  const out: string[] = [];
  for (const line of (text ?? '').split('\n')) {
    for (const p of parts(line)) {
      if (p.kind === 'link' && !out.some((t) => t.toLowerCase() === p.title.toLowerCase())) out.push(p.title);
    }
  }
  return out;
}
