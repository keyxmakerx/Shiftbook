<script lang="ts">
  // The bottom bar (round 12): thin and out of the way. Windows line up from the left like
  // the Windows taskbar (a line under each: accent when open, grey when minimised); today's
  // hours and the bell sit on the right. The shortcut hint lives here too.
  import { bell } from '../lib/bell.svelte';
  import { fmtHours } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { app, ribbonDay } from '../lib/store.svelte';
  import { closeWindow, setWindowMode, toggleFromBar, windows, winKey } from '../lib/windows.svelte';
  import type { WindowSpot } from '../lib/api';
  import Icon from './Icon.svelte';

  // Titles come from the windows once they have loaded; until then a short kind name.
  const KIND: Record<string, string> = { entry: 'Entry notepad', tray: 'Ticket notepad', note: 'Note', inbox: 'Scrap' };

  const today = $derived(ribbonDay(app.today)?.minutes ?? 0);
  const count = $derived(bell.items.length);
  const urgent = $derived(bell.items.some((i) => i.when === 'now'));

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
  </div>
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

  @media (max-width: 760px) {
    .hint {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tb,
    .bell.urgent :global(svg) {
      animation: none;
    }
  }
</style>
