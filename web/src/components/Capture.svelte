<script lang="ts">
  // Capture (Ctrl+Space): one line such as "1.5h firewall cleanup CHG0012345 #network
  // yesterday". The app reads it as you type (crates/core/src/capture.rs) and shows chips.
  // Enter logs it; Ctrl+Enter, or a line without hours, saves it to the Inbox.
  import { api } from '../lib/api';
  import type { Capture } from '../lib/api';
  import { dayOfMonth, fmtMinutes, weekdayShort } from '../lib/format';
  import { app, attempt, flashEntry, projectById, projectColor, refresh, refreshInbox, selectDay, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let input: HTMLInputElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (app.captureOpen) {
      text = '';
      seen = null;
      queueMicrotask(() => input?.focus());
    }
  });

  function read() {
    if (timer !== null) clearTimeout(timer);
    const now = text;
    if (!now.trim()) {
      seen = null;
      return;
    }
    timer = setTimeout(async () => {
      const c = await api.parseCapture(now, app.today).catch(() => null);
      if (text === now) seen = c;
    }, 60);
  }

  function close() {
    app.captureOpen = false;
  }

  async function toInbox(line: string) {
    const item = await attempt(() => api.addInbox(line));
    if (!item) return;
    close();
    await attempt(refreshInbox);
    toast('Saved to the Inbox', 'Undo', () => {
      void attempt(() => api.setInboxDeleted(item.id, true)).then(() => attempt(refreshInbox));
    });
  }

  async function submit(toInboxAnyway: boolean) {
    const line = text.trim();
    if (!line) return;
    const parsed = seen ?? (await api.parseCapture(line, app.today).catch(() => null));
    if (toInboxAnyway || !parsed || parsed.minutes === null) {
      await toInbox(line);
      return;
    }
    const entry = await attempt(() => api.captureLog(line, app.today));
    if (!entry) return;
    close();
    if (app.view !== 'desk') setView('desk');
    await selectDay(entry.date);
    await refresh();
    flashEntry(entry.id);
    const when = entry.date === app.today ? 'today' : `${weekdayShort(entry.date)} ${dayOfMonth(entry.date)}`;
    toast(`Logged ${fmtMinutes(parsed.minutes)} on ${when}`, 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    });
  }

  function dayText(c: Capture): string {
    return c.date === app.today ? 'today' : `${weekdayShort(c.date)} ${dayOfMonth(c.date)}`;
  }
</script>

<div class="scrim" class:on={app.captureOpen} onclick={close} aria-hidden="true"></div>
<div class="paper capture" class:on={app.captureOpen} role="dialog" aria-label="Log or note something" aria-hidden={!app.captureOpen}>
  <input
    bind:this={input}
    bind:value={text}
    oninput={read}
    onkeydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        close();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        void submit(e.ctrlKey || e.metaKey);
      }
    }}
    tabindex={app.captureOpen ? 0 : -1}
    autocomplete="off"
    spellcheck="false"
    aria-label="Capture line"
    placeholder="Try: 1.5h firewall cleanup CHG0012345 #network yesterday"
  />
  <div class="toks" aria-live="polite">
    {#if seen && text.trim()}
      {#if seen.minutes !== null}
        <span class="tok" style:--c="var(--ok)"><em>time</em>{fmtMinutes(seen.minutes)}</span>
      {/if}
      {#if seen.projectId}
        <span class="tok" style:--c={projectColor(seen.projectId)}><em>project</em>{projectById(seen.projectId)?.name}</span>
      {:else if seen.projectQuery}
        <span class="tok" style:--c="var(--warn)"><em>project?</em>no “{seen.projectQuery}”</span>
      {/if}
      {#each seen.tickets as t (t)}
        <span class="tok" style:--c="var(--accent)"><em>ticket</em>{t}</span>
      {/each}
      <span class="tok"><em>day</em>{dayText(seen)}</span>
      {#if seen.description}
        <span class="tok" style:--c="var(--ink)"><em>what</em>{seen.description}</span>
      {/if}
      {#if seen.minutes === null}
        <span class="tok" style:--c="var(--warn)"><em>no hours</em>Enter saves it to the Inbox</span>
      {/if}
    {/if}
  </div>
  <div class="foot">
    <span><span class="kbd">Enter</span> log it</span>
    <span><span class="kbd">Ctrl Enter</span> to Inbox</span>
    <span><span class="kbd">Esc</span> close</span>
    <span class="tip">Hours, #project, tickets and days are picked up as you type</span>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: color-mix(in srgb, var(--desk) 40%, transparent);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.2s;
  }

  .scrim.on {
    opacity: 1;
    pointer-events: auto;
  }

  .capture {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 92px;
    width: min(660px, calc(100% - 32px));
    transform: translate(-50%, -14px) scale(0.96);
    opacity: 0;
    pointer-events: none;
    visibility: hidden;
    transition:
      transform calc(0.35s * var(--m)) var(--spring),
      opacity 0.2s,
      visibility 0s 0.2s;
    overflow: hidden;
    padding: 0;
  }

  .capture.on {
    transform: translateX(-50%);
    opacity: 1;
    pointer-events: auto;
    visibility: visible;
    transition:
      transform calc(0.35s * var(--m)) var(--spring),
      opacity 0.2s;
  }

  input {
    width: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    padding: 16px 18px;
    font: 500 18px var(--ui);
  }

  .toks {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    padding: 0 18px 12px;
    min-height: 32px;
  }

  .tok {
    font: 600 12px var(--ui);
    padding: 3px 9px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--c, var(--ink)) 13%, transparent);
    color: color-mix(in srgb, var(--c, var(--ink)) 85%, var(--ink));
    animation: pop 0.3s var(--bounce);
  }

  .tok em {
    font-style: normal;
    opacity: 0.65;
    font-weight: 500;
    margin-right: 4px;
  }

  @keyframes pop {
    from {
      transform: scale(0.6);
      opacity: 0;
    }
  }

  .foot {
    display: flex;
    gap: 16px;
    padding: 9px 18px;
    border-top: 1px solid var(--rule);
    font-size: 12px;
    color: var(--mut);
    flex-wrap: wrap;
  }

  .tip {
    margin-left: auto;
  }
</style>
