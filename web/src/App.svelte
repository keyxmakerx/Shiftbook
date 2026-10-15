<script lang="ts">
  import { api } from './lib/api';
  import { errorMessage } from './lib/messages';
  import { checkIdle, recordActivity, startIdle, TICK_MS } from './lib/idle';
  import { lockAndReload } from './lib/lock';
  import Setup from './pages/Setup.svelte';
  import RecoveryKey from './pages/RecoveryKey.svelte';
  import Unlock from './pages/Unlock.svelte';
  import Today from './pages/Today.svelte';
  import ChangePassword from './components/ChangePassword.svelte';

  type Screen = 'loading' | 'problem' | 'setup' | 'recovery' | 'unlock' | 'app';

  let screen = $state<Screen>('loading');
  let problem = $state('');
  // Held only between setup and "Continue" on the recovery key page.
  let recoveryKey = $state<string | null>(null);
  let showChangePassword = $state(false);
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

  const unlocked = $derived(screen === 'app' || screen === 'recovery');

  async function lockNow() {
    if (locking) return;
    locking = true;
    recoveryKey = null;
    await lockAndReload();
  }

  // Idle auto-lock (15 minutes) and lock after the computer sleeps, while unlocked.
  $effect(() => {
    if (!unlocked) return;
    let idle = startIdle(Date.now());
    const onActivity = () => {
      idle = recordActivity(idle, Date.now());
    };
    const events = ['keydown', 'pointerdown', 'pointermove', 'wheel', 'touchstart'] as const;
    for (const name of events) window.addEventListener(name, onActivity, { passive: true });
    const timer = setInterval(() => {
      const result = checkIdle(idle, Date.now());
      idle = result.state;
      if (result.lock) void lockNow();
    }, TICK_MS);
    return () => {
      clearInterval(timer);
      for (const name of events) window.removeEventListener(name, onActivity);
    };
  });

  function onKeydown(event: KeyboardEvent) {
    if (screen !== 'app') return;
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
      screen = 'app';
    }}
  />
{:else if screen === 'unlock'}
  <Unlock onUnlocked={() => (screen = 'app')} />
{:else if screen === 'app'}
  <header class="topbar">
    <span class="brand">Shiftbook</span>
    <nav aria-label="Security">
      <button type="button" onclick={() => (showChangePassword = !showChangePassword)}>
        Change password
      </button>
      <button type="button" class="primary" onclick={lockNow} disabled={locking}>
        Lock now <kbd>Ctrl+L</kbd>
      </button>
    </nav>
  </header>
  {#if showChangePassword}
    <div class="narrow">
      <ChangePassword onClose={() => (showChangePassword = false)} />
    </div>
  {/if}
  <Today />
{/if}
