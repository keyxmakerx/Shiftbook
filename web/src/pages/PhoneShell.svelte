<script lang="ts">
  // The phone layout (round 15): today's hours on top, one view at a time, the views on a bar
  // along the bottom and a big + for capture. Same store and components as the computer
  // layout; the Desk canvas stays on the computer, so Desk is today as one list.
  import { onMount } from 'svelte';
  import { bell } from '../lib/bell.svelte';
  import { dayOfMonth, fmtHours, longDay, relativeDay, weekdayShort } from '../lib/format';
  import type { IconName } from '../lib/icons';
  import type { View } from '../lib/keys';
  import { app, ribbonDay, ribbonWeekOf, selectDay, setView, shownViews } from '../lib/store.svelte';
  import { followDay, startShell } from '../lib/shellLife.svelte';
  import BellPanel from '../components/BellPanel.svelte';
  import Capture from '../components/Capture.svelte';
  import ShareSheet from '../components/ShareSheet.svelte';
  import { openCapture, planNudges, takeLaunch } from '../lib/launch.svelte';
  import Drawer from '../components/Drawer.svelte';
  import Icon from '../components/Icon.svelte';
  import PaperMenu from '../components/PaperMenu.svelte';
  import TimerBar from '../components/TimerBar.svelte';
  import Toast from '../components/Toast.svelte';
  import WindowLayer from '../components/WindowLayer.svelte';
  import Day from '../views/Day.svelte';
  import Lists from '../views/Lists.svelte';
  import Notes from '../views/Notes.svelte';
  import Repeating from '../views/Repeating.svelte';
  import Settings from '../views/Settings.svelte';
  import Timesheet from '../views/Timesheet.svelte';

  let { onLock }: { onLock: () => void } = $props();
  let problem = $state('');

  onMount(() => startShell((message) => (problem = message)));
  followDay();

  // Why Android opened Shiftbook (shared text, a shortcut, the tile, a nudge): once loaded,
  // and whenever it comes back to the front. The nudges are planned again once loaded.
  $effect(() => {
    if (!app.ready) return;
    void takeLaunch();
    void planNudges();
    const onVisible = () => {
      if (document.visibilityState === 'visible') void takeLaunch();
    };
    document.addEventListener('visibilitychange', onVisible);
    return () => document.removeEventListener('visibilitychange', onVisible);
  });

  const VIEW_TABS: Record<View, { label: string; icon: IconName }> = {
    desk: { label: 'Desk', icon: 'desk' },
    day: { label: 'Day', icon: 'calendar' },
    notes: { label: 'Notes', icon: 'note' },
    timesheet: { label: 'Timesheet', icon: 'sheet' },
    repeating: { label: 'Repeating', icon: 'repeat' },
    lists: { label: 'Lists', icon: 'list' },
    settings: { label: 'Settings', icon: 'sliders' },
  };

  // Four views on the bar (Timesheet at work, Repeating at home) and the rest under More.
  const shown = $derived(shownViews());
  const barViews = $derived(
    (['desk', 'day', 'notes', 'timesheet', 'repeating'] as View[]).filter((v) => shown.includes(v)).slice(0, 4),
  );
  const moreViews = $derived(shown.filter((v) => !barViews.includes(v)));
  let moreOpen = $state(false);

  // A view this mode doesn't show (say Timesheet after switching to home) falls back to Desk.
  $effect(() => {
    if (app.ready && !shown.includes(app.view)) setView('desk');
  });

  const todayMinutes = $derived(ribbonDay(app.today)?.minutes ?? 0);
  const weekMinutes = $derived(ribbonWeekOf(app.today)?.minutes ?? 0);
  const dueCount = $derived(bell.items.length);
  /** The last two weeks, newest last, for the Day view's strip. */
  const recentDays = $derived(
    app.ribbon
      .flatMap((w) => w.days)
      .filter((d) => !d.future)
      .slice(-14),
  );

  // Desk is always today; Day keeps whatever day you picked.
  $effect(() => {
    if (app.view === 'desk' && app.selected !== app.today) void selectDay(app.today);
  });

  let strip: HTMLElement | undefined = $state();
  $effect(() => {
    void app.selected;
    strip?.querySelector('[aria-pressed="true"]')?.scrollIntoView({ inline: 'center', block: 'nearest' });
  });

  function go(view: View) {
    bell.open = false;
    moreOpen = false;
    setView(view);
  }
</script>

<div class="phone">
  <header class="top">
    <div class="when">
      <h1>{app.view === 'day' ? longDay(app.selected) : longDay(app.today)}</h1>
      <span class="sum">
        {#if app.view === 'day' && app.selected !== app.today}
          {relativeDay(app.selected, app.today)} · <b>{fmtHours(ribbonDay(app.selected)?.minutes ?? 0)} h</b>
        {:else}
          Today <b>{fmtHours(todayMinutes)} h</b> · Week <b>{fmtHours(weekMinutes)} h</b>
        {/if}
      </span>
    </div>
    <button
      class="round"
      class:hot={dueCount > 0}
      type="button"
      aria-label={dueCount ? `${dueCount} due soon` : 'Nothing due'}
      onclick={() => (bell.open = !bell.open)}
    >
      <Icon name="bell" size={20} />{#if dueCount}<b>{dueCount}</b>{/if}
    </button>
    <button class="round" type="button" aria-label="Lock Shiftbook" onclick={onLock}>
      <Icon name="lock" size={20} />
    </button>
  </header>
  <TimerBar />

  {#if app.view === 'day' && app.ready}
    <div class="strip" bind:this={strip} aria-label="Pick a day">
      {#each recentDays as d (d.date)}
        <button
          type="button"
          aria-pressed={d.date === app.selected}
          class:gap={d.gap}
          aria-label="{longDay(d.date)}, {d.gap ? 'nothing logged' : `${d.hours} hours`}"
          onclick={() => void selectDay(d.date)}
        >
          <small>{weekdayShort(d.date)}</small><b>{dayOfMonth(d.date)}</b><small>{d.gap ? '·' : d.hours}</small>
        </button>
      {/each}
    </div>
  {/if}

  <main class="stage">
    {#if problem}
      <p class="error pad" role="alert">{problem}</p>
    {:else if app.ready}
      <section class="view" aria-label={app.view}>
        {#if app.view === 'desk' || app.view === 'day'}
          <Day />
        {:else if app.view === 'timesheet'}
          <Timesheet />
        {:else if app.view === 'notes'}
          <Notes />
        {:else if app.view === 'repeating'}
          <Repeating />
        {:else if app.view === 'lists'}
          <Lists />
        {:else}
          <Settings />
        {/if}
      </section>
    {:else}
      <p class="pad lead">Opening your work log…</p>
    {/if}
    {#if app.ready}
      <WindowLayer />
      <BellPanel />
    {/if}
  </main>

  {#if app.ready && (app.view === 'desk' || app.view === 'day')}
    <button class="fab" type="button" aria-label="Log or jot something" onclick={() => openCapture()}>
      <Icon name="plus" size={28} />
    </button>
  {/if}

  {#if moreOpen}
    <button class="scrim" type="button" aria-label="Close" onclick={() => (moreOpen = false)}></button>
    <div class="more" role="menu" aria-label="More views">
      {#each moreViews as v (v)}
        <button type="button" role="menuitem" aria-current={app.view === v} onclick={() => go(v)}>
          <Icon name={VIEW_TABS[v].icon} size={22} /><span>{VIEW_TABS[v].label}</span>
        </button>
      {/each}
    </div>
  {/if}

  <nav class="tabs" aria-label="Views">
    {#each barViews as v (v)}
      <button type="button" aria-pressed={app.view === v} onclick={() => go(v)}>
        <Icon name={VIEW_TABS[v].icon} size={22} /><span>{VIEW_TABS[v].label}</span>
      </button>
    {/each}
    <button
      type="button"
      aria-pressed={moreViews.includes(app.view)}
      aria-expanded={moreOpen}
      onclick={() => (moreOpen = !moreOpen)}
    >
      <Icon name="more" size={22} /><span>More</span>
    </button>
  </nav>
</div>

<Capture {onLock} />
<Drawer />
<PaperMenu />
<ShareSheet />
<Toast />

<style>
  .phone {
    height: 100vh;
    height: 100dvh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding-top: env(safe-area-inset-top, 0px);
  }

  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px 8px;
  }

  .when {
    flex: 1;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font: 650 21px/1.2 var(--display);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sum {
    color: var(--mut);
    font-variant-numeric: tabular-nums;
  }

  .sum b {
    color: var(--ink);
  }

  .round {
    position: relative;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink-2);
    display: grid;
    place-items: center;
    flex: none;
  }

  .round.hot {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, var(--rule));
  }

  .round b {
    position: absolute;
    top: -3px;
    right: -3px;
    min-width: 19px;
    height: 19px;
    padding: 0 5px;
    border-radius: 10px;
    background: var(--warn);
    color: var(--paper);
    font-size: 11px;
    display: grid;
    place-items: center;
  }

  .strip {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding: 2px 14px 10px;
    scrollbar-width: none;
  }

  .strip button {
    flex: none;
    width: 48px;
    padding: 6px 0;
    border-radius: 12px;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    font-variant-numeric: tabular-nums;
  }

  .strip button small {
    font-size: 11px;
    color: var(--mut);
  }

  .strip button.gap {
    border-style: dashed;
  }

  .strip button[aria-pressed='true'] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  .strip button[aria-pressed='true'] small {
    color: inherit;
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .view {
    padding: 4px 10px 96px;
  }

  .fab {
    position: fixed;
    right: 16px;
    bottom: calc(76px + env(safe-area-inset-bottom, 0px));
    width: 60px;
    height: 60px;
    border-radius: 18px;
    border: 0;
    background: var(--accent);
    color: var(--accent-fg);
    display: grid;
    place-items: center;
    box-shadow: var(--lift);
    z-index: 20;
  }

  .tabs {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    background: var(--paper);
    border-top: 1px solid var(--rule);
    padding: 6px 4px calc(8px + env(safe-area-inset-bottom, 0px));
    z-index: 21;
  }

  .scrim {
    position: fixed;
    inset: 0;
    border: 0;
    background: color-mix(in srgb, var(--ink) 18%, transparent);
    z-index: 22;
  }

  .more {
    position: fixed;
    right: 10px;
    bottom: calc(78px + env(safe-area-inset-bottom, 0px));
    min-width: 200px;
    background: var(--paper);
    border: 1px solid var(--rule);
    border-radius: 14px;
    box-shadow: var(--lift);
    padding: 6px;
    display: flex;
    flex-direction: column;
    z-index: 23;
  }

  .more button {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 0 14px;
    border: 0;
    border-radius: 10px;
    background: none;
    color: var(--ink);
    font-size: 16px;
    text-align: left;
  }

  .more button[aria-current='true'] {
    color: var(--accent);
    font-weight: 600;
  }

  .more button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .tabs button {
    border: 0;
    background: none;
    color: var(--mut);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 6px 0;
    min-height: 52px;
    font-size: 12px;
  }

  .tabs button[aria-pressed='true'] {
    color: var(--accent);
    font-weight: 600;
  }

  .tabs button:focus-visible,
  .round:focus-visible,
  .fab:focus-visible,
  .strip button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
