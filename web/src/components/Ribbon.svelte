<script lang="ts">
  // The frosted time ribbon: brand, capture box, protection pill, Lock, the view switcher
  // and the last four weeks as days of coloured hours (click a day to open it).
  import { api } from '../lib/api';
  import type { RibbonDay } from '../lib/api';
  import { dayOfMonth, longDay, weekdayShort } from '../lib/format';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { allowCapture, countdown, protection, protectNow } from '../lib/protection.svelte';
  import { app, attempt, flashEntry, projectColor, refresh, selectDay, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import type { View } from '../lib/keys';
  import Icon from './Icon.svelte';

  let { onLock, onCapture }: { onLock: () => void; onCapture: () => void } = $props();

  // Planner and Canvas join this list, in their places, once they're built (round 7).
  const VIEW_LIST: { id: View; label: string; key: string }[] = [
    { id: 'desk', label: 'Desk', key: '1' },
    { id: 'timesheet', label: 'Timesheet', key: '2' },
    { id: 'customize', label: 'Customize', key: '3' },
  ];

  let viewsEl: HTMLElement | undefined = $state();
  let pill = $state({ left: 0, width: 0 });
  let grown = $state(false);

  $effect(() => {
    void app.view;
    void viewsEl;
    const place = () => {
      const b = viewsEl?.querySelector<HTMLElement>('button[aria-pressed="true"]');
      if (b) pill = { left: b.offsetLeft, width: b.offsetWidth };
    };
    place();
    window.addEventListener('resize', place);
    return () => window.removeEventListener('resize', place);
  });

  $effect(() => {
    const t = setTimeout(() => (grown = true), 1600);
    return () => clearTimeout(t);
  });

  const MAX_BAR_MINUTES = 9 * 60;

  function partHeight(minutes: number): number {
    return Math.min(36, (minutes / MAX_BAR_MINUTES) * 36);
  }

  function dayMenu(day: RibbonDay): MenuSpec | null {
    if (day.future) return null;
    return {
      sections: [
        {
          head: `${longDay(day.date)} · ${day.hours} h`,
          items: [
            { label: 'Open this day', icon: 'desk', side: true, act: () => openDay(day.date) },
            {
              label: 'Log something for this day',
              icon: 'log',
              side: true,
              act: () => {
                void selectDay(day.date).then(onCapture);
              },
            },
            {
              label: 'What to enter for this week',
              icon: 'sheet',
              top: true,
              act: () => {
                void selectDay(day.date).then(() => setView('timesheet'));
              },
            },
            {
              label: 'Book as holiday (8 h)',
              icon: 'sun',
              top: true,
              act: () => void holiday(day.date),
            },
          ],
        },
      ],
    };
  }

  async function holiday(date: string) {
    const entry = await attempt(() =>
      api.createEntry(
        {
          date,
          minutes: 480,
          description: 'Holiday',
          projectId: null,
          category: null,
          notes: null,
          timesheetCodeId: null,
        },
        '',
      ),
    );
    if (!entry) return;
    await refresh();
    flashEntry(entry.id);
    toast(`${weekdayShort(date)} ${dayOfMonth(date)} booked as holiday`, 'Undo', () => {
      void attempt(() => api.deleteEntry(entry.id)).then(refresh);
    });
  }

  function openDay(date: string) {
    setView('desk');
    void selectDay(date);
  }

  async function toggleProtection() {
    await attempt(async () => {
      if (protection.on) {
        await allowCapture();
        toast('Screenshots and copying are allowed for 10 minutes.', 'Protect now', () => {
          void attempt(protectNow);
        });
      } else {
        await protectNow();
        toast('Protected again. Screenshots and copying are blocked.');
      }
    });
  }
</script>

<header class="ribbon">
  <div class="brand"><b>Shiftbook</b><span class="lbl">alpha</span></div>
  <div class="acts">
    <button class="cap-btn" type="button" onclick={onCapture} title="Log, note or find (Ctrl+Space)">
      <span>Log or note something…</span><kbd class="kbd">Ctrl Space</kbd>
    </button>
    <button
      class="icon-btn prot"
      class:open={!protection.on}
      type="button"
      onclick={toggleProtection}
      title={protection.on
        ? 'Screenshots, screen sharing and copying are blocked. Click to allow for 10 minutes.'
        : 'Allowed for now. Click to protect again.'}
    >
      <i></i><span>{protection.on ? 'Protected' : `Allowed ${countdown(protection.secondsLeft)}`}</span>
    </button>
    <button class="icon-btn" type="button" onclick={onLock} title="Lock now (Ctrl+L)">
      <Icon name="lock" size={15} /><span>Lock</span>
    </button>
  </div>
  <nav class="views" aria-label="Views" bind:this={viewsEl}>
    <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
    {#each VIEW_LIST as v (v.id)}
      <button type="button" aria-pressed={app.view === v.id} title="{v.label} ({v.key})" onclick={() => setView(v.id)}>
        {v.label}
      </button>
    {/each}
  </nav>
  <div class="days" class:grow={!grown} aria-label="Last four weeks">
    {#each app.ribbon as week, w (week.weekStart)}
      <div class="wk" class:sel={week.days.some((d) => d.date === app.selected)}>
        {#each week.days as day, i (day.date)}
          <button
            type="button"
            class="d"
            class:now={day.date === app.today}
            class:on={day.date === app.selected}
            class:future={day.future}
            disabled={day.future}
            title="{longDay(day.date)} · {day.hours} h{day.future ? '' : ' · right-click for options'}"
            aria-label="{longDay(day.date)}, {day.hours} hours"
            onclick={() => openDay(day.date)}
            use:paperMenu={() => dayMenu(day)}
          >
            <span class="bar" style:--k={w * 7 + i}>
              {#each day.parts as part, pi (pi)}
                <i style:height="{partHeight(part.minutes)}px" style:background-color={projectColor(part.projectId)}></i>
              {/each}
            </span>
            <small>{dayOfMonth(day.date)}</small>
          </button>
        {/each}
      </div>
    {/each}
  </div>
</header>

<style>
  .ribbon {
    position: relative;
    z-index: 10;
    margin: 10px 12px 0;
    background: var(--glass);
    backdrop-filter: blur(18px) saturate(1.5);
    -webkit-backdrop-filter: blur(18px) saturate(1.5);
    border: 1px solid var(--glass-line);
    border-radius: 14px;
    box-shadow: var(--shadow);
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 6px 16px;
    align-items: center;
    padding: 8px 12px;
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .brand b {
    font: 650 19px var(--display);
    letter-spacing: -0.01em;
  }

  .views {
    display: inline-flex;
    background: color-mix(in srgb, var(--ink) 7%, transparent);
    border-radius: 9px;
    padding: 3px;
    position: relative;
    justify-self: start;
  }

  .views button {
    position: relative;
    z-index: 1;
    border: 0;
    background: none;
    padding: 5px 12px;
    border-radius: 7px;
    cursor: pointer;
    color: var(--ink-2);
    font-weight: 500;
    transition: color 0.2s;
  }

  .views button[aria-pressed='true'] {
    color: var(--ink);
  }

  .pill {
    position: absolute;
    z-index: 0;
    top: 3px;
    bottom: 3px;
    border-radius: 7px;
    background: var(--paper);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
    transition:
      left calc(0.4s * var(--m)) var(--spring),
      width calc(0.4s * var(--m)) var(--spring);
  }

  .days {
    grid-column: 1 / -1;
    display: flex;
    gap: 3px;
    align-items: flex-end;
    height: 64px;
    overflow-x: auto;
    scrollbar-width: none;
    padding-top: 4px;
  }

  .wk {
    display: flex;
    gap: 3px;
    padding: 3px 4px 2px;
    border-radius: 8px;
    transition: background 0.3s;
  }

  .wk.sel {
    background: color-mix(in srgb, var(--accent) 13%, transparent);
  }

  .d {
    width: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    cursor: pointer;
    border: 0;
    background: none;
    padding: 0;
    color: inherit;
    border-radius: 5px;
  }

  .d:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .bar {
    width: 15px;
    height: 36px;
    display: flex;
    flex-direction: column-reverse;
    border-radius: 3px;
    overflow: hidden;
    background: color-mix(in srgb, var(--ink) 6%, transparent);
    transition: transform 0.2s var(--bounce);
    transform-origin: bottom;
  }

  .d:hover:not(:disabled) .bar {
    transform: scaleY(1.08);
  }

  .bar i {
    display: block;
    flex: none;
    transition:
      height calc(0.5s * var(--m)) var(--spring),
      background-color 0.4s;
  }

  .grow .bar {
    animation: grow calc(0.7s * var(--m)) var(--spring) both;
    animation-delay: calc(var(--k) * 14ms);
  }

  @keyframes grow {
    from {
      transform: scaleY(0);
    }
  }

  .d small {
    font: 500 10px var(--mono);
    color: var(--mut);
    padding: 0 3px;
    border-radius: 4px;
  }

  .d.now small {
    background: var(--ink);
    color: var(--desk);
  }

  .d.on small {
    outline: 1.5px solid var(--accent);
  }

  .acts {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-self: end;
  }

  .cap-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 230px;
    border: 1px solid var(--rule);
    background: var(--paper);
    border-radius: 9px;
    padding: 6px 10px;
    cursor: text;
    color: var(--mut);
  }

  .cap-btn kbd {
    margin-left: auto;
  }

  .icon-btn {
    border: 1px solid var(--rule);
    background: var(--paper);
    border-radius: 9px;
    padding: 6px 10px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 12.5px;
    min-height: 32px;
  }

  .prot i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 25%, transparent);
  }

  .prot.open i {
    background: var(--warn);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--warn) 25%, transparent);
    animation: pulse 1.6s infinite;
  }

  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 6px color-mix(in srgb, var(--warn) 10%, transparent);
    }
  }

  @media (max-width: 1080px) {
    .ribbon {
      grid-template-columns: 1fr auto;
    }
    .views {
      grid-column: 1 / -1;
      order: 3;
    }
    .cap-btn {
      min-width: 0;
    }
    .cap-btn span {
      display: none;
    }
  }

  @media (max-width: 760px) {
    .ribbon {
      grid-template-columns: 1fr;
    }
    .acts {
      flex-wrap: wrap;
      justify-self: start;
    }
  }
</style>
