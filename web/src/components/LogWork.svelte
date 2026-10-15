<script lang="ts">
  // Log work: paste a ticket or say what you did, then press a time button. The app
  // reuses today's entry for a ticket it already has (see crates/core/src/quicklog.rs).
  import { api } from '../lib/api';
  import type { Capture } from '../lib/api';
  import { fmtMinutes } from '../lib/format';
  import { app, attempt, flashEntry, projectById, projectColor, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  const TIMES = [15, 30, 45, 60, 120];

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let input: HTMLInputElement | undefined = $state();
  let working = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const serial = app.logFocus.serial;
    if (serial === 0) return;
    if (app.logFocus.text) {
      text = app.logFocus.text;
      recognise();
    }
    queueMicrotask(() => input?.focus());
  });

  function recognise() {
    if (timer !== null) clearTimeout(timer);
    const now = text;
    if (!now.trim()) {
      seen = null;
      return;
    }
    timer = setTimeout(async () => {
      const c = await api.parseCapture(now, app.selected).catch(() => null);
      if (text === now) seen = c;
    }, 120);
  }

  async function log(minutes: number) {
    if (!text.trim()) {
      input?.focus();
      toast('Paste a ticket or type what you did first.');
      return;
    }
    working = true;
    const before = new Map(app.entries.map((e) => [e.id, e.minutes]));
    const entry = await attempt(() => api.quickLog(app.selected, text, minutes));
    working = false;
    if (!entry) return;
    text = '';
    seen = null;
    await refresh();
    flashEntry(entry.id);
    const existed = before.has(entry.id);
    toast(
      existed
        ? `Added ${fmtMinutes(minutes)} to ${entry.description}`
        : `Logged ${fmtMinutes(minutes)}${entry.tickets.length ? ' on ' + entry.tickets.map((t) => t.number).join(', ') : ''}`,
      'Undo',
      () => {
        const undo: Promise<unknown> = existed ? api.addEntryMinutes(entry.id, -minutes) : api.deleteEntry(entry.id);
        void attempt(() => undo).then(refresh);
      },
    );
  }
</script>

<div class="quick">
  <label class="lbl" for="log-work">Log work · paste a ticket or say what you did, then pick the time</label>
  <div class="in">
    <input
      id="log-work"
      class="text-in"
      bind:this={input}
      bind:value={text}
      oninput={recognise}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          toast('Pick a time button to log it.');
        }
      }}
      placeholder="INC0045678 or “printer on 2nd floor”"
      autocomplete="off"
      spellcheck="false"
    />
    {#each TIMES as m (m)}
      <button type="button" class="hbtn" disabled={working} onclick={() => log(m)}>{fmtMinutes(m)}</button>
    {/each}
  </div>
  <div class="recog" aria-live="polite">
    {#if seen && text.trim()}
      {#each seen.tickets as t (t)}
        <span class="chip" style:--c={projectColor(seen.projectId)}>{t}</span>
      {/each}
      {#if seen.projectId}
        <span>goes to <b>{projectById(seen.projectId)?.name}</b></span>
      {:else if seen.projectQuery}
        <span class="warn">no project called “{seen.projectQuery}”</span>
      {/if}
      {#if seen.description}<span>as “{seen.description}”</span>{/if}
      <span class="lbl pick">pick a time →</span>
    {/if}
  </div>
</div>

<style>
  .quick {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .in {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .in input {
    flex: 1 1 220px;
    font-size: 14.5px;
    padding: 8px 11px;
  }

  .recog {
    font-size: 12.5px;
    color: var(--ink-2);
    min-height: 20px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .pick {
    margin-left: auto;
  }

  .warn {
    color: var(--warn);
  }
</style>
