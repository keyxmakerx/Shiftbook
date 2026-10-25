<script lang="ts">
  // The unlock screen: the password or the recovery key, or (round 15) today's PIN while
  // Quick unlock's window is open. "Use my password instead" ends the window.
  import { api } from '../lib/api';
  import type { SecretKind } from '../lib/api';
  import { toIsoDate } from '../lib/dates';
  import { SLEEP_GAP_MS, TICK_MS } from '../lib/idle';
  import { errorMessage } from '../lib/messages';
  import DeleteAllData from '../components/DeleteAllData.svelte';
  import PinBoxes from '../components/PinBoxes.svelte';

  let {
    onUnlocked,
  }: { onUnlocked: (mustSetPassword: boolean, withPin: boolean) => void } = $props();

  let usePin = $state(false);
  let pin = $state('');
  let pinNote = $state('');
  let boxes: PinBoxes | undefined = $state();

  $effect(() => {
    void api
      .quickUnlockReady(toIsoDate(new Date()))
      .then((ready) => (usePin = ready))
      .catch(() => (usePin = false));
  });

  // A computer that slept while locked ends the PIN window, as it does while unlocked.
  $effect(() => {
    if (!usePin) return;
    let last = Date.now();
    const timer = setInterval(() => {
      const now = Date.now();
      if (now - last >= SLEEP_GAP_MS) void passwordInstead();
      last = now;
    }, TICK_MS);
    return () => clearInterval(timer);
  });

  async function passwordInstead() {
    usePin = false;
    pin = '';
    await api.endQuickUnlock().catch(() => undefined);
  }

  async function tryPin(typed: string) {
    if (working) return;
    pin = '';
    error = '';
    working = true;
    try {
      const outcome = await api.unlockWithPin(typed, toIsoDate(new Date()));
      if (outcome.unlocked) {
        onUnlocked(false, true);
        return;
      }
      if (outcome.triesLeft === 0) {
        usePin = false;
        pinNote = 'That was the third wrong PIN. Enter your password to open Shiftbook.';
      } else {
        error = `That PIN isn't right. ${outcome.triesLeft} ${outcome.triesLeft === 1 ? 'try' : 'tries'} left, then your password.`;
      }
    } catch {
      // The window ended (midnight, 10 hours or sleep): ask for the password.
      usePin = false;
      pinNote = 'Quick unlock has ended for now. Enter your password.';
    } finally {
      working = false;
      if (usePin) queueMicrotask(() => boxes?.focus());
    }
  }

  let kind = $state<SecretKind>('password');
  let secret = $state('');
  let working = $state(false);
  let error = $state('');
  let input: HTMLInputElement | undefined = $state();
  // Lost both the password and the recovery key: offer to delete everything.
  let startingOver = $state(false);

  $effect(() => {
    // Focus the field on open and whenever the mode switches.
    void kind;
    input?.focus();
  });

  function switchKind() {
    kind = kind === 'password' ? 'recovery_key' : 'password';
    secret = '';
    error = '';
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (working || secret.length === 0) return;
    const typed = secret;
    secret = '';
    error = '';
    working = true;
    try {
      const outcome = await api.unlock(typed, kind);
      onUnlocked(outcome.mustSetPassword, false);
    } catch (e) {
      error = errorMessage(e);
      input?.focus();
    } finally {
      working = false;
    }
  }
</script>

<main class="narrow">
  {#if startingOver}
    <DeleteAllData locked onCancel={() => (startingOver = false)} />
  {:else if usePin}
    <h1>Locked while you were away</h1>
    <p class="lead">Enter your PIN · 3 tries, then your password.</p>
    <div class="stack pin-form">
      <PinBoxes bind:this={boxes} bind:value={pin} label="PIN" disabled={working} autofocus onfull={(p) => void tryPin(p)} />
      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      <button type="button" class="link" onclick={() => void passwordInstead()} disabled={working}>
        Use my password instead
      </button>
    </div>
  {:else}
    <h1>Shiftbook is locked</h1>
    <p class="lead">
      {kind === 'password'
        ? 'Enter your password to open your work log.'
        : 'Enter the recovery key you saved when you set up Shiftbook.'}
    </p>

    {#if pinNote}
      <p class="help" role="status">{pinNote}</p>
    {/if}
    <form onsubmit={submit} class="stack">
      <div class="field">
        <label for="unlock-secret">{kind === 'password' ? 'Password' : 'Recovery key'}</label>
        <input
          bind:this={input}
          id="unlock-secret"
          class:mono={kind === 'recovery_key'}
          type={kind === 'password' ? 'password' : 'text'}
          bind:value={secret}
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          disabled={working}
          aria-describedby={kind === 'recovery_key' ? 'recovery-help' : undefined}
        />
        {#if kind === 'recovery_key'}
          <p id="recovery-help" class="help">
            52 letters and numbers. Spaces, dashes and upper or lower case don't matter.
          </p>
        {/if}
      </div>

      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}

      <button type="submit" class="primary" disabled={working || secret.length === 0}>
        {working ? 'Unlocking…' : 'Unlock'}
      </button>
      <button type="button" class="link" onclick={switchKind} disabled={working}>
        {kind === 'password' ? 'Forgot your password? Use your recovery key' : 'Use your password instead'}
      </button>
      {#if kind === 'recovery_key'}
        <button type="button" class="link" onclick={() => (startingOver = true)} disabled={working}>
          Lost your recovery key too? Delete everything and start again
        </button>
      {/if}
    </form>
  {/if}
</main>

<style>
  .pin-form {
    align-items: flex-start;
  }
</style>
