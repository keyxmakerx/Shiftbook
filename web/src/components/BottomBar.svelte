<script lang="ts">
  // The bottom bar (round 12): thin and out of the way. Windows line up from the left like
  // the Windows taskbar (a line under each: accent when open, grey when minimised); pinned
  // chips (round 14) come next, orange once their follow-up day comes; today's hours and the
  // bell sit on the right. The shortcut hint lives here too.
  import { bell } from '../lib/bell.svelte';
  import { fmtHours, relativeDay } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { chipMenu, pins } from '../lib/pins.svelte';
  import { app, ribbonDay, words } from '../lib/store.svelte';
  import { closeWindow, openWindow, setWindowMode, toggleFromBar, windows, winKey } from '../lib/windows.svelte';
  import type { PinItem, WindowSpot } from '../lib/api';
  import Icon from './Icon.svelte';
  import { book, closeMopBook, setBookMode, toggleBookFromBar, tabInfo } from '../lib/mopbook.svelte';

  // Titles come from the windows once they have loaded; until then a short kind name.
  const KIND: Record<string, string> = { entry: 'Entry notepad', tray: 'Ticket notepad', note: 'Note', inbox: 'Scrap' };

  const today = $derived(ribbonDay(app.today)?.minutes ?? 0);
  const count = $derived(bell.items.length);
  const urgent = $derived(bell.items.some((i) => i.when === 'now'));

  function pinTip(p: PinItem): string {
    const when = !p.chaseOn
      ? 'no follow-up day'
      : p.chaseOn === app.today
        ? 'follow up today'
        : p.due
          ? `follow up since ${relativeDay(p.chaseOn, app.today)}`
          : `follow up ${relativeDay(p.chaseOn, app.today)}`;
    return `${p.title} · ${when} · click to open, right-click to follow up or unpin`;
  }

  function menu(e: MouseEvent, w: WindowSpot) {
    e.preventDefault();
    const key = winKey(w);
    openMenu(e.currentTarget as HTMLElement, {
      sections: [
        {
          items: [
            { label: w.mode === 'min' ? 'Show it' : 'Minimise', icon: w.mode === 'min' ? 'open' : 'minus', act: () => setWindowMode(key, w.mode === 'min' ? 'open' : 'min') },
            { label: 'Roll up', icon: 'up', act: () => setWindowMode(key, 'rolled') },
            { label: 'Full screen', icon: 'expand', act: () => setWindowMode(key, 'full') },
            { label: 'Close', icon: 'x', danger: true, act: () => closeWindow(key) },
          ],
        },
      ],
    });
  }
</script>

<footer class="bar" aria-label="Bottom bar">
  <div class="wins" role="toolbar" aria-label="Windows">
    {#each windows.list as w (winKey(w))}
      {@const key = winKey(w)}
      <button
        type="button"
        class="tb"
        class:min={w.mode === 'min'}
        class:front={windows.front === key && w.mode !== 'min'}
        title={w.mode === 'min' ? 'Show' : windows.front === key ? 'Minimise' : 'Bring to the front'}
        onclick={() => toggleFromBar(key)}
        oncontextmenu={(e) => menu(e, w)}
      >
        <span class="ic" class:note={w.kind === 'note'} class:tray={w.kind === 'tray'} aria-hidden="true"></span>
        <span class="label">{windows.titles[key] ?? KIND[w.kind]}</span>
      </button>
    {/each}
    {#if book.shown}
      <button
        type="button"
        class="tb"
        class:min={book.mode === 'min'}
        class:front={book.mode !== 'min'}
        title={book.mode === 'min' ? 'Show the notebook' : 'Minimise the notebook'}
        onclick={toggleBookFromBar}
        oncontextmenu={(e) => {
          e.preventDefault();
          openMenu(e.currentTarget as HTMLElement, {
            sections: [
              {
                items: [
                  { label: book.mode === 'min' ? 'Show it' : 'Minimise', icon: book.mode === 'min' ? 'open' : 'minus', act: toggleBookFromBar },
                  { label: 'Roll up', icon: 'up', act: () => setBookMode('rolled') },
                  { label: 'Full screen', icon: 'expand', act: () => setBookMode('full') },
                  { label: 'Close', icon: 'x', danger: true, act: closeMopBook },
                ],
              },
            ],
          });
        }}
      >
        <span class="ic book" aria-hidden="true"></span>
        <span class="label">{book.active && tabInfo.titles[book.active] ? `${words().book} · ${tabInfo.titles[book.active]}` : words().book}</span>
      </button>
    {/if}
  </div>
  {#if pins.list.length}
    <div class="pins" role="toolbar" aria-label="Pinned">
      {#each pins.list as p (p.kind + p.id)}
        <button
          type="button"
          class="pin"
          class:due={p.due}
          title={pinTip(p)}
          onclick={() => openWindow(p.kind, p.id)}
          oncontextmenu={(e) => {
            e.preventDefault();
            openMenu(e.currentTarget as HTMLElement, chipMenu(p));
          }}
        >
          <Icon name="pin" size={12} />
          <span class="label">{p.title}</span>
        </button>
      {/each}
    </div>
  {/if}
  {#if app.view === 'desk'}
    <button
      type="button"
      class="today"
      class:on={app.justToday}
      aria-pressed={app.justToday}
      title="Fade cards that aren't from today, still open, brought up or pinned (T)"
      onclick={() => (app.justToday = !app.justToday)}
    >◐ Just today <span class="kbd">T</span></button>
    {#if app.justToday}<span class="hint" role="status">Showing today, still open, brought up and pinned</span>{/if}
  {/if}
  <span class="hint" aria-hidden="true">Hold <span class="kbd">Ctrl</span> for shortcuts</span>
  <span class="hours mono" title="Logged today">{fmtHours(today)} h today</span>
  <button
    type="button"
    class="bell"
    class:on={bell.open}
    class:urgent
    aria-label={count ? `Coming up: ${count}` : 'Coming up: nothing'}
    aria-expanded={bell.open}
    title="Coming up"
    onclick={() => (bell.open = !bell.open)}
  >
    <Icon name="bell" size={17} />
    {#if count}<span class="badge">{count}</span>{/if}
  </button>
</footer>

<style>
  .bar {
    flex: none;
    height: 40px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    background: var(--glass);
    border-top: 1px solid var(--rule);
    backdrop-filter: blur(10px);
    position: relative;
    z-index: 45;
  }

  .today {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: none;
    color: var(--ink-2);
    font: 600 12.5px var(--ui);
    cursor: pointer;
    white-space: nowrap;
  }

  .today.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  .wins {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 4px;
    overflow: hidden;
  }

  .tb {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    max-width: 200px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ink);
    font: 550 12.5px var(--ui);
    cursor: pointer;
    animation: slidein calc(0.25s * var(--m)) var(--spring);
  }

  @keyframes slidein {
    from {
      opacity: 0;
      transform: translateX(-12px);
    }
  }

  .tb:hover,
  .tb.front {
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }

  .tb::after {
    content: '';
    position: absolute;
    left: 30%;
    right: 30%;
    bottom: 2px;
    height: 2.5px;
    border-radius: 2px;
    background: var(--accent);
    transition: left 0.2s, right 0.2s;
  }

  .tb.min::after {
    left: 44%;
    right: 44%;
    background: var(--mut);
  }

  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ic {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    background: var(--accent);
    flex: none;
  }

  .ic.note {
    background: var(--warn);
  }

  .ic.tray {
    background: var(--ok);
  }

  .ic.book {
    background: #8a4c7b;
  }

  .pins {
    display: flex;
    gap: 4px;
    min-width: 0;
    max-width: 55%;
    overflow: hidden;
    flex-shrink: 1;
  }

  .pin {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    max-width: 190px;
    min-width: 60px;
    padding: 0 10px 0 8px;
    border: 1px solid var(--rule);
    border-radius: 13px;
    background: var(--paper);
    color: var(--ink);
    font: 550 12px var(--ui);
    cursor: pointer;
    animation: slidein calc(0.25s * var(--m)) var(--spring);
  }

  .pin:hover {
    border-color: color-mix(in srgb, var(--ink) 25%, transparent);
  }

  .pin.due {
    background: #e07b1a;
    border-color: #e07b1a;
    color: #fff;
  }

  .hint {
    font-size: 11.5px;
    color: var(--mut);
    white-space: nowrap;
  }

  .hours {
    font: 600 12px var(--mono);
    color: var(--mut);
    padding: 0 6px;
    white-space: nowrap;
  }

  .bell {
    position: relative;
    width: 36px;
    height: 30px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ink);
    cursor: pointer;
  }

  .bell:hover,
  .bell.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .bell.urgent :global(svg) {
    animation: ring calc(1.2s * var(--m)) var(--ease) 2;
    transform-origin: 50% 10%;
  }

  @keyframes ring {
    0%,
    100% {
      transform: rotate(0);
    }
    20% {
      transform: rotate(14deg);
    }
    40% {
      transform: rotate(-12deg);
    }
    60% {
      transform: rotate(6deg);
    }
  }

  .badge {
    position: absolute;
    top: 1px;
    right: 2px;
    min-width: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--bad);
    color: #fff;
    font: 700 10px/15px var(--ui);
  }

  @media (max-width: 1100px) {
    .hint {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tb,
    .pin,
    .bell.urgent :global(svg) {
      animation: none;
    }
  }
</style>
