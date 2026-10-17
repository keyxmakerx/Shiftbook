<script lang="ts">
  // Log work: paste a ticket or say what you did, then press a time button. The app
  // reuses today's entry for a ticket it already has (see crates/core/src/quicklog.rs).
  import { api } from '../lib/api';
  import type { Capture } from '../lib/api';
  import type { Entry } from '../lib/api';
  import { addDays, fmtMinutes } from '../lib/format';
  import { repeatCandidates } from '../lib/repeat';
  import { app, attempt, flashEntry, projectById, projectColor, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  const TIMES = [15, 30, 45, 60, 120];

  let text = $state('');
  let seen = $state<Capture | null>(null);
  let input: HTMLInputElement | undefined = $state();
  let working = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let timesEl: HTMLElement | undefined = $state();
  let earlier = $state<Entry[]>([]);

  // The week before the selected day, for "Same as yesterday".
  $effect(() => {
    const day = app.selected;
    let live = true;
    api
      .listEntries(addDays(day, -7), addDays(day, -1))
      .then((list) => {
        if (live) earlier = list;
      })
      .catch(() => {
        if (live) earlier = [];
      });
    return () => {
      live = false;
    };
  });

  const repeats = $derived(repeatCandidates(app.selected, earlier, app.entries));

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

  /** Enter: log the line when it has a time in it, otherwise move to the time buttons. */
  async function enter() {
    if (!text.trim()) return;
    const now = text;
    const c = await api.parseCapture(now, app.selected).catch(() => null);
    if (text !== now) return;
    seen = c;
    if (c?.minutes && c.hoursText) {
      await log(c.minutes, now.replace(c.hoursText, ' ').replace(/\s+/g, ' ').trim());
    } else {
      timesEl?.querySelector<HTMLButtonElement>('button')?.focus();
    }
  }

  /** Left and right arrows move between the time buttons. */
  function timesKey(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    const list = [...(timesEl?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    event.preventDefault();
    if (event.key === 'ArrowLeft' && i === 0) input?.focus();
    else list[Math.min(list.length - 1, Math.max(0, i + (event.key === 'ArrowRight' ? 1 : -1)))]?.focus();
  }

  async function repeat(e: Entry) {
    working = true;
    const entry = await attempt(() =>
      api.createEntry(
        {
          date: app.selected,
          minutes: e.minutes,
          description: e.description,
          projectId: e.projectId,
          category: e.category,
          notes: null,
          timesheetCodeId: e.timesheetCodeId,
        },
        '',
      ),
    );
    working = false;
    if (!entry) return;
    await refresh();
    flashEntry(entry.id);
    toast(`Logged ${fmtMinutes(e.minutes)} of ${e.description}`, 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    });
  }

  async function log(minutes: number, line = text) {
    if (!line.trim()) {
      input?.focus();
      toast('Paste a ticket or type what you did first.');
      return;
    }
    working = true;
    const before = new Map(app.entries.map((e) => [e.id, e.minutes]));
    const entry = await attempt(() => api.quickLog(app.selected, line, minutes));
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
  <label class="lbl" for="log-work">Log work · paste a ticket or say what you did, add a time or pick one</label>
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
          void enter();
        } else if (e.key === 'ArrowRight' && input && input.selectionStart === text.length && text) {
          e.preventDefault();
          timesEl?.querySelector<HTMLButtonElement>('button')?.focus();
        }
      }}
      placeholder="INC0045678 50m or “printer on 2nd floor”"
      autocomplete="off"
      spellcheck="false"
    />
    <span class="times" role="toolbar" tabindex="-1" aria-label="Log it as" bind:this={timesEl} onkeydown={timesKey}>
      {#each TIMES as m (m)}
        <button type="button" class="hbtn" disabled={working} onclick={() => log(m)}>{fmtMinutes(m)}</button>
      {/each}
    </span>
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
      {#if seen.minutes}
        <span class="lbl pick"><b class="mono">{fmtMinutes(seen.minutes)}</b> · <span class="kbd">Enter</span> logs it</span>
      {:else}
        <span class="lbl pick">pick a time, or <span class="kbd">Enter</span> to jump to them</span>
      {/if}
    {/if}
  </div>
  {#if repeats.length > 0}
    <div class="repeat">
      <span class="lbl">Same as yesterday</span>
      {#each repeats as r (r.id)}
        <button type="button" class="btn rep" disabled={working} onclick={() => repeat(r)} title="Log {fmtMinutes(r.minutes)} of {r.description} again">
          <span class="chip" style:--c={projectColor(r.projectId)}>{fmtMinutes(r.minutes)}</span>{r.description}
        </button>
      {/each}
    </div>
  {/if}
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
    text-transform: none;
    letter-spacing: 0;
  }

  .times {
    display: inline-flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .repeat {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .repeat .lbl {
    margin-right: 2px;
  }

  .rep {
    font-weight: 500;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warn {
    color: var(--warn);
  }
</style>
