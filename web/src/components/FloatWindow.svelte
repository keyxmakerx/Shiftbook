<script lang="ts">
  // One window (round 12): a notepad or a note floating over Shiftbook. Drag the title bar to
  // move it, the corner to resize it. ▴ rolls it up to the title bar, – tucks it into the
  // bottom bar, ⤢ fills the screen, ✕ closes it. What's typed saves as you go.
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { Note, WindowSpot } from '../lib/api';
  import { app, attempt, projectById, projectColor, putEntry } from '../lib/store.svelte';
  import { closeWindow, focusWindow, moveWindow, resizeWindow, setWindowMode, windows, winKey } from '../lib/windows.svelte';
  import Icon from './Icon.svelte';
  import { phone } from '../lib/phone.svelte';
  import { dragButton } from '../lib/rightDrag';

  let { win }: { win: WindowSpot } = $props();

  const SAVE_AFTER_MS = 700;
  const key = $derived(winKey(win));
  const front = $derived(windows.front === key);

  // What the window shows. It loads again when something changes elsewhere, unless there's
  // typing here not saved yet; a save first checks nothing else wrote the notepad meanwhile.
  let title = $state('');
  let sub = $state('');
  let color = $state('var(--accent)');
  let text = $state('');
  let saved = '';
  let note: Note | null = null;
  let isGone = $state(false);
  let status = $state<'' | 'saving' | 'saved'>('');
  /** The newer text written somewhere else while this window had unsaved typing. */
  let clash = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  // Live position and size while dragging; saved when let go.
  let live = $state<{ x: number; y: number; w: number; h: number } | null>(null);

  // Loading happens in the effect below (on opening, and after changes elsewhere).
  onMount(() => {
    return () => {
      if (timer || clash !== null) {
        if (timer) clearTimeout(timer);
        void save(true);
      }
    };
  });

  // Something changed elsewhere (a card, the tray, a sync): show it, unless typing is waiting.
  $effect(() => {
    void app.changed;
    untrack(() => {
      if (!timer && clash === null && text === saved) void load();
    });
  });

  /** Load what the window shows; false when it's gone. */
  async function load() {
    const now = await current();
    if (now === null) return;
    text = saved = now;
  }

  /** The text as stored now (and the title and colour), or null when it's gone. */
  async function current(): Promise<string | null> {
    if (win.kind === 'entry') {
      const e = await attempt(() => api.getEntry(win.id));
      if (!e) return gone(true);
      title = e.description || 'Untitled';
      sub = projectById(e.projectId)?.name ?? 'No project';
      color = projectColor(e.projectId);
      return e.notes ?? '';
    } else if (win.kind === 'tray') {
      const t = await attempt(() => api.trayItem(win.id));
      if (!t) return gone(true);
      title = t.ticket;
      sub = t.done ? 'Ticket' : 'Ticket · still open';
      color = t.projectId ? projectColor(t.projectId) : 'var(--accent)';
      return t.note ?? '';
    } else if (win.kind === 'note') {
      const n = await attempt(() => api.getNote(win.id));
      if (!n) return gone(true);
      note = n;
      title = n.title;
      sub = `${n.kind[0]!.toUpperCase()}${n.kind.slice(1)} note`;
      color = n.projectId ? projectColor(n.projectId) : 'var(--warn)';
      return n.body;
    }
    return gone(true);
  }

  function gone(yes: true): null {
    isGone = yes;
    return null;
  }

  /**
   * Save the typing. If the notepad was written somewhere else since this window loaded it,
   * nothing is overwritten: the window asks which to keep. `closing` keeps both instead (the
   * newer text, then this window's), as there's no window left to ask in.
   */
  async function save(closing = false) {
    timer = null;
    if (text === saved && clash === null) return;
    const stored = await current();
    if (stored === null) return;
    let body = text;
    if (stored !== saved && stored !== text) {
      if (!closing) {
        clash = stored;
        status = '';
        return;
      }
      body = `${stored}\n\n${text}`;
    }
    clash = null;
    status = 'saving';
    let ok = false;
    if (win.kind === 'entry') {
      const e = await attempt(() => api.setEntryNotes(win.id, body));
      if (e) putEntry(e);
      ok = !!e;
    } else if (win.kind === 'tray') {
      const day = await attempt(() => api.traySetNote(win.id, body));
      if (day && app.tray?.date === day.date) app.tray = day;
      ok = !!day;
    } else if (win.kind === 'note' && note) {
      const n = note;
      const updated = await attempt(() =>
        api.updateNote(n.id, { kind: n.kind, title: n.title, body, projectId: n.projectId, tags: n.tags.join(', ') }),
      );
      if (updated) note = updated;
      ok = !!updated;
    }
    if (ok) saved = body;
    status = ok ? 'saved' : '';
  }

  /** Overwrite the newer text with this window's. */
  function keepMine() {
    if (clash === null) return;
    saved = clash;
    clash = null;
    void save();
  }

  /** Drop this window's typing and show the newer text. */
  function useNewer() {
    if (clash === null) return;
    text = saved = clash;
    clash = null;
  }

  function typed() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void save(), SAVE_AFTER_MS);
  }

  function drag(e: PointerEvent, resize: boolean) {
    if (!dragButton(e) || win.mode === 'full') return;
    if (!resize && (e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    focusWindow(key);
    const start = { x: e.clientX, y: e.clientY };
    const from = { x: win.x, y: win.y, w: win.w, h: win.h };
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      const dx = m.clientX - start.x;
      const dy = m.clientY - start.y;
      live = resize
        ? { ...from, w: Math.max(260, from.w + dx), h: Math.max(160, from.h + dy) }
        : { ...from, x: from.x + dx, y: Math.max(0, from.y + dy) };
    };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
      if (live) {
        if (resize) resizeWindow(key, live.w, live.h);
        else moveWindow(key, live.x, live.y);
      }
      live = null;
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }

  const box = $derived(live ?? { x: win.x, y: win.y, w: win.w, h: win.h });

  $effect(() => {
    if (title) windows.titles[key] = title;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section
  class="win paper"
  class:front
  class:rolled={win.mode === 'rolled'}
  class:full={win.mode === 'full'}
  class:moving={!!live}
  hidden={win.mode === 'min'}
  style:--c={color}
  style:left="{box.x}px"
  style:top="{box.y}px"
  style:width="{box.w}px"
  style:height={win.mode === 'rolled' ? 'auto' : `${box.h}px`}
  aria-label="{title || 'Window'} window"
  onpointerdown={() => !front && focusWindow(key)}
>
  <header
    class="bar"
    onpointerdown={(e) => drag(e, false)}
    ondblclick={() => !phone.on && setWindowMode(key, win.mode === 'full' ? 'open' : 'full')}
  >
    <span class="ic" aria-hidden="true"></span>
    <span class="ttl">{title || '…'}</span>
    <span class="sub">{sub}</span>
    <!-- A phone shows one window at a time, full screen: only Close. -->
    {#if !phone.on}
      <button
        type="button"
        title={win.mode === 'rolled' ? 'Open it again' : 'Roll up to the title bar'}
        aria-label={win.mode === 'rolled' ? 'Unroll' : 'Roll up'}
        onclick={() => setWindowMode(key, win.mode === 'rolled' ? 'open' : 'rolled')}
      >
        <Icon name={win.mode === 'rolled' ? 'down' : 'up'} size={14} />
      </button>
      <button type="button" title="Minimise to the bottom bar" aria-label="Minimise" onclick={() => setWindowMode(key, 'min')}>
        <Icon name="minus" size={14} />
      </button>
      <button
        type="button"
        title={win.mode === 'full' ? 'Back to a window' : 'Full screen'}
        aria-label={win.mode === 'full' ? 'Restore' : 'Full screen'}
        onclick={() => setWindowMode(key, win.mode === 'full' ? 'open' : 'full')}
      >
        <Icon name={win.mode === 'full' ? 'restore' : 'expand'} size={14} />
      </button>
    {/if}
    <button type="button" class="x" title="Close" aria-label="Close" onclick={() => closeWindow(key)}>
      <Icon name="x" size={14} />
    </button>
  </header>
  {#if win.mode !== 'rolled'}
    {#if isGone}
      <p class="gone">This is no longer here. It may have been deleted or taken out of the tray.</p>
    {:else}
      <textarea
        class="body"
        bind:value={text}
        placeholder={win.kind === 'note' ? 'Write here…' : 'Notes…'}
        aria-label="{title} notepad"
        oninput={typed}
        onblur={() => {
          if (timer) clearTimeout(timer);
          void save();
        }}
      ></textarea>
      <footer>
        {#if clash !== null}
          <span class="clash" role="alert">
            Changed somewhere else since you opened this.
            <button type="button" onclick={keepMine}>Keep mine</button>
            <button type="button" onclick={useNewer}>Show the newer one</button>
          </span>
        {:else}
          <span aria-live="polite">{status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved' : 'Saves as you type'}</span>
        {/if}
        {#if win.mode !== 'full'}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span class="grip" title="Drag to resize" onpointerdown={(e) => drag(e, true)}></span>
        {/if}
      </footer>
    {/if}
  {/if}
</section>

<style>
  .win {
    position: absolute;
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    overflow: hidden;
    box-shadow:
      0 0 0 1px var(--rule),
      0 26px 50px -22px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    animation: pop calc(0.22s * var(--m)) var(--spring);
    transition:
      left calc(0.25s * var(--m)) var(--ease),
      top calc(0.25s * var(--m)) var(--ease),
      width calc(0.25s * var(--m)) var(--ease),
      height calc(0.25s * var(--m)) var(--ease);
  }

  .clash button {
    margin-left: 6px;
  }

  .win.moving {
    transition: none;
  }

  .win[hidden] {
    display: none;
  }

  .win.front {
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--c) 45%, var(--rule)),
      0 30px 60px -20px rgba(0, 0, 0, 0.6);
  }

  .win.full {
    left: 0 !important;
    top: 0 !important;
    width: 100% !important;
    height: 100% !important;
    border-radius: 0;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.97);
    }
  }

  .bar {
    flex: none;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px 0 12px;
    background: color-mix(in srgb, var(--c) 10%, var(--paper));
    border-bottom: 1px solid var(--rule);
    cursor: grab;
    user-select: none;
    touch-action: none;
  }

  .full .bar {
    cursor: default;
  }

  .ic {
    width: 11px;
    height: 11px;
    border-radius: 3px;
    background: var(--c);
    flex: none;
  }

  .ttl {
    font-weight: 650;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .sub {
    font-size: 11.5px;
    color: var(--mut);
    white-space: nowrap;
    margin-right: auto;
  }

  .bar button {
    width: 28px;
    height: 24px;
    border: 0;
    background: none;
    border-radius: 6px;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
    flex: none;
  }

  .bar button:hover {
    background: color-mix(in srgb, var(--ink) 9%, transparent);
    color: var(--ink);
  }

  .bar .x:hover {
    background: var(--bad);
    color: #fff;
  }

  .body {
    flex: 1;
    min-height: 0;
    resize: none;
    border: 0;
    padding: 14px 16px;
    font: 14px/1.6 var(--ui);
    color: var(--ink);
    background: var(--paper);
  }

  .body:focus {
    outline: none;
  }

  .full .body {
    padding: 22px max(24px, calc(50% - 380px));
    font-size: 15px;
  }

  footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    font-size: 11px;
    color: var(--mut);
    border-top: 1px solid var(--rule);
    background: var(--paper);
  }

  .grip {
    margin-left: auto;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    border-right: 2px solid var(--line-strong);
    border-bottom: 2px solid var(--line-strong);
    border-radius: 0 0 4px 0;
    touch-action: none;
  }

  .gone {
    padding: 16px;
    color: var(--mut);
    margin: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .win {
      animation: none;
      transition: none;
    }
  }
</style>
