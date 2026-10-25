<script lang="ts">
  import { api } from './lib/api';
  import { errorMessage } from './lib/messages';
  import { checkIdle, recordActivity, startIdle, TICK_MS } from './lib/idle';
  import { lockAndReload } from './lib/lock';
  import { app } from './lib/store.svelte';
  import Setup from './pages/Setup.svelte';
  import RecoveryKey from './pages/RecoveryKey.svelte';
  import Unlock from './pages/Unlock.svelte';
  import NewPassword from './pages/NewPassword.svelte';
  import Purpose from './pages/Purpose.svelte';
  import Shell from './pages/Shell.svelte';
  import PhoneShell from './pages/PhoneShell.svelte';
  import { phone } from './lib/phone.svelte';
  import { timerBack, timerHidden } from './lib/timer.svelte';
  import { planNudges } from './lib/launch.svelte';

  type Screen = 'loading' | 'problem' | 'setup' | 'recovery' | 'purpose' | 'unlock' | 'new-password' | 'app';

  let screen = $state<Screen>('loading');
  let problem = $state('');
  // Held only between setup and "Continue" on the recovery key page.
  let recoveryKey = $state<string | null>(null);
  let locking = $state(false);

  $effect(() => {
    api
      .vaultStatus()
      .then((status) => {
        screen = status === 'needs_setup' ? 'setup' : status === 'locked' ? 'unlock' : 'app';
      })
      .catch((e: unknown) => {
        problem = errorMessage(e);
        screen = 'problem';
      });
  });

  const unlocked = $derived(screen === 'app' || screen === 'recovery' || screen === 'purpose' || screen === 'new-password');

  async function lockNow() {
    if (locking) return;
    locking = true;
    recoveryKey = null;
    await lockAndReload();
  }

  // Idle auto-lock (15 minutes unless changed in Settings → Security) and lock after the
  // computer sleeps, while unlocked.
  $effect(() => {
    if (!unlocked) return;
    const idleMs = (app.prefs?.autoLockMinutes ?? 15) * 60 * 1000;
    let idle = startIdle(Date.now());
    const onActivity = () => {
      idle = recordActivity(idle, Date.now());
    };
    const events = ['keydown', 'pointerdown', 'pointermove', 'wheel', 'touchstart'] as const;
    for (const name of events) window.addEventListener(name, onActivity, { passive: true });
    const timer = setInterval(() => {
      const result = checkIdle(idle, Date.now(), idleMs);
      idle = result.state;
      if (result.lock) void lockNow();
    }, TICK_MS);
    return () => {
      clearInterval(timer);
      for (const name of events) window.removeEventListener(name, onActivity);
    };
  });

  // Phone: lock after being away from the app (another app, the home screen or the screen off)
  // for the chosen time; 0 locks as soon as Shiftbook is left.
  $effect(() => {
    if (!unlocked || !phone.on) return;
    const awayMs = (app.prefs?.leaveLockMinutes ?? 1) * 60 * 1000;
    let leftAt: number | null = null;
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') {
        leftAt = Date.now();
        // The timer goes into the notification first, so locking keeps it as away.
        timerHidden();
        if (screen === 'app') void planNudges();
        if (awayMs === 0) void lockNow();
      } else if (leftAt !== null && Date.now() - leftAt >= awayMs) {
        // After unlock the timer picks up and asks about the time away.
        void lockNow();
      } else {
        leftAt = null;
        void timerBack();
      }
    };
    document.addEventListener('visibilitychange', onVisibility);
    return () => document.removeEventListener('visibilitychange', onVisibility);
  });

  function onKeydown(event: KeyboardEvent) {
    // In the app the Shell handles Ctrl+L; elsewhere while unlocked, lock here.
    if (screen === 'app' || !unlocked) return;
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      void lockNow();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if screen === 'loading'}
  <main class="narrow"><p class="lead">Opening Shiftbook…</p></main>
{:else if screen === 'problem'}
  <main class="narrow">
    <h1>Shiftbook can't start</h1>
    <p class="error" role="alert">{problem}</p>
  </main>
{:else if screen === 'setup'}
  <Setup
    onDone={(key) => {
      recoveryKey = key;
      screen = 'recovery';
    }}
  />
{:else if screen === 'recovery' && recoveryKey !== null}
  <RecoveryKey
    {recoveryKey}
    onDone={() => {
      recoveryKey = null;
      screen = 'purpose';
    }}
  />
{:else if screen === 'purpose'}
  <Purpose onDone={() => (screen = 'app')} />
{:else if screen === 'unlock'}
  <Unlock onUnlocked={(mustSetPassword) => (screen = mustSetPassword ? 'new-password' : 'app')} />
{:else if screen === 'new-password'}
  <NewPassword onDone={() => (screen = 'app')} />
{:else if screen === 'app'}
  {#if phone.on}
    <PhoneShell onLock={lockNow} />
  {:else}
    <Shell onLock={lockNow} />
  {/if}
{/if}
