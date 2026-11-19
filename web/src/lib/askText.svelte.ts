// A small box that asks for one line of text (round 21: the name for a saved response's
// {name} blank). One at a time; `askText` resolves with what was typed, or null when closed.

export const textPrompt = $state({ open: false, title: '', label: '', hint: '', ok: 'OK', value: '', serial: 0 });

let settle: ((text: string | null) => void) | null = null;

/** Ask for a line of text; `value` is what the box starts with (a name being changed). */
export function askText(ask: { title: string; label: string; hint?: string; ok?: string; value?: string }): Promise<string | null> {
  settle?.(null);
  textPrompt.title = ask.title;
  textPrompt.label = ask.label;
  textPrompt.hint = ask.hint ?? '';
  textPrompt.ok = ask.ok ?? 'OK';
  textPrompt.value = ask.value ?? '';
  textPrompt.open = true;
  textPrompt.serial += 1;
  return new Promise((resolve) => {
    settle = resolve;
  });
}

/** Close the box with what was typed, or null. */
export function answerText(text: string | null): void {
  textPrompt.open = false;
  const done = settle;
  settle = null;
  done?.(text);
}
