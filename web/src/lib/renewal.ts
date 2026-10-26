// Start renewal (round 15, card r15-repeating): the Log work line that files the work under a
// repeating thing. The `#tag` is the thing's name with its spaces made dashes, which the
// capture rules (crates/core/src/capture.rs) match to the thing whatever else is in the line.

/** "Renew www.example.com certificate #www.example.com-certificate " (ends ready to type). */
export function startLine(name: string, expires: boolean): string {
  const clean = name.trim().replace(/\s+/g, ' ');
  const tag = clean.replace(/[\s#]+/g, '-');
  return `${expires ? `Renew ${clean}` : clean} #${tag} `;
}
