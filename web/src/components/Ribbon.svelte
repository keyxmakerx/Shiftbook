<script lang="ts">
  // The frosted time ribbon: brand, capture box, protection pill, Lock, the view switcher
  // and the last four weeks as days of coloured hours (click a day to open it).
  import { api } from '../lib/api';
  import type { RibbonDay } from '../lib/api';
  import { dayOfMonth, longDay, weekdayShort } from '../lib/format';
  import { paperMenu, type MenuSpec } from '../lib/menu.svelte';
  import { allowCapture, countdown, guardedWords, guards, protection, protectNow } from '../lib/protection.svelte';
  import { bell } from '../lib/bell.svelte';
  import { app, attempt, flashEntry, focusLog, projectColor, refresh, selectDay, setView, shownTabs } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { ORGANISE, VIEW_NAMES, type View } from '../lib/keys';
  import type { IconName } from '../lib/icons';
  import { hasWork } from '../lib/words';
  import Icon from './Icon.svelte';

  let { onLock, onCapture }: { onLock: () => void; onCapture: () => void } = $props();

  // Desk is the Canvas, where the day happens; Day is for starting and ending it (round 9).
  // Round 18: tabs are only where you work; the places you go to look after your things sit
  // in the Organise menu after them, and Settings is the gear by Lock. The Timesheet hides at
  // home. Notes moved from the menu to a tab beside the Wiki (asked in chat, 6 Nov 2026). The
  // Planner joins the tabs once it's built.
  const VIEW_LIST = $derived(shownTabs().map((id, i) => ({ id, label: VIEW_NAMES[id], key: String(i + 1) })));
  const organiseKey = $derived(String(VIEW_LIST.length + 1));
  const inOrganise = $derived(ORGANISE.includes(app.view));

  const ORGANISE_ROWS: Record<string, { icon: IconName; sub: string }> = {
    items: { icon: 'ticket', sub: 'Open, followed up and done' },
    projects: { icon: 'box', sub: 'Hours, tickets and the board for each' },
    repeating: { icon: 'repeat', sub: 'Renewals, chores and regular events' },
    lists: { icon: 'list', sub: 'Checklists you reset and use again' },
  };

  /** A count beside each row: open tickets and things due, from the bell. */
  function count(v: View): { n: number; warn: boolean } | null {
    if (v === 'items') {
      const n = bell.items.reduce((sum, i) => sum + (i.about.kind === 'still_open' ? i.about.tickets.length : 0), 0);
      return n ? { n, warn: false } : null;
    }
    if (v === 'repeating') {
      const n = bell.items.filter((i) => i.about.kind === 'due').length;
      return n ? { n, warn: true } : null;
    }
    return null;
  }

  let organiseBtn: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLElement | undefined = $state();

  function go(v: View) {
    app.organiseOpen = false;
    setView(v);
  }

  // Opening the menu (click, or the number key after the tabs) puts focus on its first row.
  $effect(() => {
    if (!app.organiseOpen) return;
    queueMicrotask(() => menuEl?.querySelector<HTMLElement>('[role="menuitem"]')?.focus());
    const away = (e: PointerEvent) => {
      if (!menuEl?.contains(e.target as Node) && !organiseBtn?.contains(e.target as Node)) app.organiseOpen = false;
    };
    window.addEventListener('pointerdown', away, true);
    return () => window.removeEventListener('pointerdown', away, true);
  });

  function menuKeys(e: KeyboardEvent) {
    const rows = [...(menuEl?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const at = rows.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      app.organiseOpen = false;
      organiseBtn?.focus();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const step = e.key === 'ArrowDown' ? 1 : -1;
      rows[(at + step + rows.length) % rows.length]?.focus();
    } else if (e.key.length === 1 && /[a-z]/i.test(e.key)) {
      const hit = rows.find((r) => r.dataset.name?.toLowerCase().startsWith(e.key.toLowerCase()));
      if (hit) {
        e.preventDefault();
        hit.click();
      }
    }
  }

  let viewsEl: HTMLElement | undefined = $state();
  let pill = $state({ left: 0, width: 0 });
  let grown = $state(false);

  $effect(() => {
    void app.view;
    void viewsEl;
    void VIEW_LIST;
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
            { label: 'Open this day', icon: 'desk', act: () => openDay(day.date) },
            {
              label: 'Log something for this day',
              icon: 'log',
              act: () => {
                void selectDay(day.date).then(onCapture);
              },
            },
            ...(hasWork(app.prefs?.useMode)
              ? [
                  {
                    label: 'Open the Timesheet for this week',
                    icon: 'sheet' as const,
                    act: () => {
                      void selectDay(day.date).then(() => setView('timesheet'));
                    },
                  },
                  {
                    label: 'Book a holiday (8 h)',
                    icon: 'sun' as const,
                    act: () => void holiday(day.date),
                  },
                ]
              : []),
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
          recurringId: null,
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
    }, () => void attempt(() => api.restoreEntry(entry.id)).then(refresh));
  }

  function openDay(date: string, gap = false) {
    setView('day');
    // A day with nothing logged opens with Log work ready to type into.
    void selectDay(date).then(() => gap && focusLog());
  }

  async function toggleProtection() {
    await attempt(async () => {
      if (protection.on) {
        await allowCapture();
        toast(`${guardedWords()} allowed for 10 minutes.`, 'Protect now', () => {
          void attempt(protectNow);
        });
      } else {
        await protectNow();
        toast(`Protected again. ${guardedWords()} blocked.`);
      }
    });
  }
</script>

<header class="ribbon" class:lifted={app.organiseOpen}>
  <div class="brand"><b>Shiftbook</b><span class="lbl">alpha</span></div>
  <div class="acts">
    <button class="cap-btn" type="button" onclick={onCapture} title="Log, note or find (Ctrl+Space)">
      <span>Log or note something…</span><kbd class="kbd">Ctrl Space</kbd>
    </button>
    {#if guards().copy || guards().screens}
    <button
      class="icon-btn prot"
      class:open={!protection.on}
      type="button"
      onclick={toggleProtection}
      title={protection.on
        ? `${guardedWords()} blocked. Click to allow for 10 minutes. Change this in Settings → Security.`
        : 'Allowed for now. Click to protect again.'}
    >
      <i></i><span>{protection.on ? 'Protected' : `Allowed ${countdown(protection.secondsLeft)}`}</span>
    </button>
    {/if}
    <button class="icon-btn" type="button" onclick={onLock} title="Lock now (Ctrl+L)">
      <Icon name="lock" size={15} /><span>Lock</span>
    </button>
    <button
      class="icon-btn gear"
      type="button"
      aria-pressed={app.view === 'settings'}
      onclick={() => setView('settings')}
      title="Settings (Ctrl+,)"
      aria-label="Settings"
    >
      <Icon name="sliders" size={15} />
    </button>
  </div>
  <nav class="views" aria-label="Views" bind:this={viewsEl}>
    <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
    {#each VIEW_LIST as v (v.id)}
      <button type="button" aria-pressed={app.view === v.id} title="{v.label} ({v.key})" onclick={() => setView(v.id)}>
        {v.label}
      </button>
    {/each}
    <button
      type="button"
      class="organise"
      bind:this={organiseBtn}
      aria-pressed={inOrganise}
      aria-haspopup="menu"
      aria-expanded={app.organiseOpen}
      title="Tickets, Projects, Repeating and Lists ({organiseKey})"
      onclick={() => (app.organiseOpen = !app.organiseOpen)}
    >
      {inOrganise ? VIEW_NAMES[app.view] : 'Organise'} <span class="caret" aria-hidden="true">▾</span>
    </button>
    {#if app.organiseOpen}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div class="paper omenu" role="menu" aria-label="Organise" tabindex="-1" bind:this={menuEl} onkeydown={menuKeys}>
        {#each ORGANISE as v, i (v)}
          {@const c = count(v)}
          <button type="button" role="menuitem" class:on={app.view === v} data-name={VIEW_NAMES[v]} onclick={() => go(v)} style:--k={i}>
            <Icon name={ORGANISE_ROWS[v]!.icon} size={16} />
            <span class="txt"><b>{VIEW_NAMES[v]}</b><small>{ORGANISE_ROWS[v]!.sub}</small></span>
            {#if c}<span class="n" class:warn={c.warn}>{c.n}{c.warn ? ' due' : ' open'}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
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
            class:gap={day.gap}
            disabled={day.future}
            title="{longDay(day.date)} · {day.gap ? 'nothing logged yet, click to log' : `${day.hours} h`}{day.future ? '' : ' · right-click for options'}"
            aria-label="{longDay(day.date)}, {day.gap ? 'nothing logged' : `${day.hours} hours`}"
            onclick={() => openDay(day.date, day.gap)}
            use:paperMenu={() => dayMenu(day)}
          >
            <span class="bar" style:--k={w * 7 + i}>
              {#each day.parts as part, pi (pi)}
                <i style:height="{partHeight(part.minutes)}px" style:background-color={projectColor(part.projectId)}></i>
              {/each}
            </span>
            <small>{dayOfMonth(day.date)}</small>
            <i class="gapdot" aria-hidden="true"></i>
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

  /* The Organise menu hangs below the ribbon, over the stage. Notepad windows (40), the bell
     (43 and 44) and the bottom bar (45) sit above the ribbon's own 10, so while the menu is open
     the ribbon rises over them; modal sheets and toasts (60 and up) still go on top. */
  .ribbon.lifted {
    z-index: 50;
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

  .views .caret {
    font-size: 10px;
    margin-left: 2px;
    color: var(--mut);
  }

  .omenu {
    position: absolute;
    z-index: 30;
    top: calc(100% + 6px);
    right: 0;
    width: 300px;
    padding: 6px;
    border-radius: 10px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: drop calc(0.2s * var(--m)) var(--spring);
    transform-origin: top right;
  }

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.97);
    }
  }

  .views .omenu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 9px;
    border-radius: 7px;
    text-align: left;
    color: var(--ink);
  }

  .views .omenu button:hover,
  .views .omenu button:focus-visible,
  .views .omenu button.on {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    outline: none;
  }

  .omenu .txt {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .omenu .txt b {
    font-weight: 600;
    font-size: 13.5px;
  }

  .omenu .txt small {
    color: var(--mut);
    font-size: 12px;
  }

  .omenu .n {
    white-space: nowrap;
    font: 600 11.5px var(--ui);
    color: var(--ink-2);
  }

  .omenu .n.warn {
    color: #fff;
    background: var(--warn);
    border-radius: 9px;
    padding: 0 7px;
  }

  .gear[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
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
    height: 70px;
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

  .gapdot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    margin-top: -1px;
  }

  .d.gap .gapdot {
    background: var(--warn);
  }

  .d.gap .bar {
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--warn) 45%, transparent);
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
