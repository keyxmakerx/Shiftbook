// "Copy for stand-up" (round 15): the text is built by the app (crates/core/src/standup.rs);
// this only copies it. Like the timesheet's Copy, it's a deliberate copy, so it works while
// Shiftbook is protected.
import { api } from './api';
import { copyText } from './protection.svelte';
import { app, attempt } from './store.svelte';
import { toast } from './toast.svelte';

export async function copyStandup(): Promise<void> {
  const text = await attempt(() => api.standup(app.today));
  if (text === undefined) return;
  const lines = text.split('\n').length;
  toast((await copyText(text)) ? `Copied ${lines} lines for stand-up` : "Couldn't copy here.");
}
