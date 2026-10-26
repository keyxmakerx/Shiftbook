<script lang="ts">
  // The unlocked app: the ribbon, one working view (Desk, Day, Timesheet, Notes or Settings) that
  // slides in from the direction you move, the windows floating over it and the bottom bar
  // with the bell, and the overlays (capture, drawer, paper menu, toast). Also the keyboard
  // shortcuts and the copy guard.
  import { onMount, untrack } from 'svelte';
  import { boards } from '../lib/boards.svelte';
  import { nextTextSize, shortcutFor, VIEWS, type View } from '../lib/keys';
  import { closeMenu, menuOpen } from '../lib/menu.svelte';
  import { scale } from '../lib/motion';
  import { app, focusLog, savePref, setView, shownViews } from '../lib/store.svelte';
  import { followDay, startShell } from '../lib/shellLife.svelte';
  import { redoLast, toast, undoLatest } from '../lib/toast.svelte';
  import BellPanel from '../components/BellPanel.svelte';
  import BoardScreen from '../components/BoardScreen.svelte';
  import BottomBar from '../components/BottomBar.svelte';
  import Capture from '../components/Capture.svelte';
  import WindowLayer from '../components/WindowLayer.svelte';
  import Drawer from '../components/Drawer.svelte';
  import PaperMenu from '../components/PaperMenu.svelte';
  import Ribbon from '../components/Ribbon.svelte';
  import TimerBar from '../components/TimerBar.svelte';
  import ShortcutOverlay from '../components/ShortcutOverlay.svelte';
  import Toast from '../components/Toast.svelte';
  import DayPrompt from '../components/DayPrompt.svelte';
  import Settings from '../views/Settings.svelte';
  import Canvas from '../views/Canvas.svelte';
  import Day from '../views/Day.svelte';
  import Timesheet from '../views/Timesheet.svelte';
  import Notes from '../views/Notes.svelte';
  import Items from '../views/Items.svelte';
  import Repeating from '../views/Repeating.svelte';
  import Lists from '../views/Lists.svelte';

  let { onLock }: { onLock: () => void } = $props();
  let problem = $state('');

  onMount(() => startShell((message) => (problem = message)));
  followDay();

  // At home the Timesheet is hidden: leave it if the mode changes while it's open.
  $effect(() => {
    if (app.ready && !shownViews().includes(app.view)) untrack(() => setView('desk'));
  });

  function order(v: View): number {
    return VIEWS.indexOf(v);
  }

  /** The views slide in the direction you move (Desk → Day → … → Settings). */
  function slide(_node: Element, { out }: { out: boolean }) {
    const k = scale();
    const dir = order(app.view) > order(app.previousView) ? 1 : -1;
    const sign = out ? -dir : dir;
    return {
      duration: (out ? 280 : 420) * k,
      easing: out ? (t: number) => t * t : (t: number) => 1 - Math.pow(1 - t, 3),
      css: (t: number) =>
        `transform: translateX(${(1 - t) * sign * 60}px) scale(${0.98 + 0.02 * t}); opacity: ${t}`,
    };
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    const typing = !!target?.closest?.('input, textarea, select, [contenteditable="true"]');
    const s = shortcutFor({
      key: event.key,
      code: event.code,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      typing,
    }, shownViews());
    if (!s) return;
    switch (s.kind) {
      case 'capture':
        event.preventDefault();
        closeMenu(true);
        app.captureOpen = !app.captureOpen;
        break;
      case 'lock':
        event.preventDefault();
        onLock();
        break;
      case 'undo':
        event.preventDefault();
        if (!undoLatest()) toast('Nothing to undo.');
        break;
      case 'redo':
        event.preventDefault();
        if (!redoLast()) toast('Nothing to redo.');
        break;
      case 'escape':
        if (menuOpen()) closeMenu();
        else if (app.captureOpen) app.captureOpen = false;
        else if (app.drawer) app.drawer = null;
        break;
      case 'just-today':
        if (app.view !== 'desk' || menuOpen() || app.captureOpen || app.drawer) return;
        event.preventDefault();
        app.justToday = !app.justToday;
        break;
      case 'focus-log':
        if (menuOpen() || app.captureOpen || app.drawer) return;
        event.preventDefault();
        focusLog();
        break;
      case 'view':
        if (menuOpen() || app.captureOpen) return;
        setView(s.view);
        break;
      case 'zoom': {
        event.preventDefault();
        const size = nextTextSize(app.prefs?.textSize ?? 100, s.step);
        if (size !== app.prefs?.textSize) {
          void savePref('ui.text_size', String(size)).then((p) => p && toast(`Text size ${size}%`));
        }
        break;
      }
      case 'menu-key':
        // Elements with a paper menu handle the Menu key themselves.
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <Ribbon {onLock} onCapture={() => (app.captureOpen = true)} />
  <TimerBar />
  <main class="stage">
    {#if problem}
      <p class="error pad" role="alert">{problem}</p>
    {:else if app.ready}
      {#key app.view}
        <section class="view" in:slide={{ out: false }} out:slide={{ out: true }} aria-label={app.view}>
          {#if app.view === 'desk'}
            <Canvas />
          {:else if app.view === 'day'}
            <Day />
          {:else if app.view === 'timesheet'}
            <Timesheet />
          {:else if app.view === 'notes'}
            <Notes />
          {:else if app.view === 'repeating'}
            <Repeating />
          {:else if app.view === 'items'}
            <Items />
          {:else if app.view === 'lists'}
            <Lists />
          {:else}
            <Settings />
          {/if}
        </section>
      {/key}
    {:else}
      <p class="pad lead">Opening your work log…</p>
    {/if}
    {#if app.ready}
      {#if boards.key}<BoardScreen />{/if}
      <WindowLayer />
      <BellPanel />
    {/if}
  </main>
  {#if app.ready}<BottomBar />{/if}
</div>

<Capture {onLock} />
<Drawer />
<PaperMenu />
<Toast />
<DayPrompt />
<ShortcutOverlay />

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .view {
    position: absolute;
    inset: 0;
    overflow: auto;
    padding: 14px 12px 24px;
  }

  .pad {
    padding: 20px;
  }

  .lead {
    color: var(--mut);
  }
</style>
