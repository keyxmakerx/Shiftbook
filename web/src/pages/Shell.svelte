<script lang="ts">
  // The unlocked app: the ribbon, one working view (Desk, Timesheet or Customize) that
  // slides in from the direction you move, and the overlays (capture, drawer, paper menu,
  // toast). Also the keyboard shortcuts and the copy guard.
  import { onMount } from 'svelte';
  import { errorMessage } from '../lib/messages';
  import { nextTextSize, shortcutFor, VIEWS, type View } from '../lib/keys';
  import { closeMenu, menuOpen } from '../lib/menu.svelte';
  import { scale } from '../lib/motion';
  import { installCopyGuard } from '../lib/protection.svelte';
  import { app, focusLog, loadAll, savePref, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Capture from '../components/Capture.svelte';
  import Drawer from '../components/Drawer.svelte';
  import PaperMenu from '../components/PaperMenu.svelte';
  import Ribbon from '../components/Ribbon.svelte';
  import ShortcutOverlay from '../components/ShortcutOverlay.svelte';
  import Toast from '../components/Toast.svelte';
  import Customize from '../views/Customize.svelte';
  import Desk from '../views/Desk.svelte';
  import Timesheet from '../views/Timesheet.svelte';

  let { onLock }: { onLock: () => void } = $props();
  let problem = $state('');

  onMount(() => {
    loadAll().catch((e: unknown) => (problem = errorMessage(e)));
    return installCopyGuard();
  });

  function order(v: View): number {
    return VIEWS.indexOf(v);
  }

  /** The views slide in the direction you move (Desk → Timesheet → Customize). */
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
    });
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
      case 'escape':
        if (menuOpen()) closeMenu();
        else if (app.captureOpen) app.captureOpen = false;
        else if (app.drawer) app.drawer = null;
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
  <main class="stage">
    {#if problem}
      <p class="error pad" role="alert">{problem}</p>
    {:else if app.ready}
      {#key app.view}
        <section class="view" in:slide={{ out: false }} out:slide={{ out: true }} aria-label={app.view}>
          {#if app.view === 'desk'}
            <Desk />
          {:else if app.view === 'timesheet'}
            <Timesheet />
          {:else}
            <Customize />
          {/if}
        </section>
      {/key}
    {:else}
      <p class="pad lead">Opening your work log…</p>
    {/if}
  </main>
</div>

<Capture {onLock} />
<Drawer />
<PaperMenu />
<Toast />
<ShortcutOverlay />
<p class="hold-hint" aria-hidden="true">Hold <span class="kbd">Ctrl</span> to see shortcuts</p>

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

  /* A quiet reminder that the shortcut overlay exists. */
  .hold-hint {
    position: fixed;
    right: 14px;
    bottom: 8px;
    margin: 0;
    font-size: 11.5px;
    color: var(--mut);
    pointer-events: none;
    opacity: 0.85;
  }

  @media (max-width: 760px) {
    .hold-hint {
      display: none;
    }
  }
</style>
