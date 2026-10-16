<script lang="ts">
  import { api } from '../lib/api';
  import type { SecretKind } from '../lib/api';
  import { errorMessage } from '../lib/messages';

  let { onUnlocked }: { onUnlocked: (mustSetPassword: boolean) => void } = $props();

  let kind = $state<SecretKind>('password');
  let secret = $state('');
  let working = $state(false);
  let error = $state('');
  let input: HTMLInputElement | undefined = $state();

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
      onUnlocked(outcome.mustSetPassword);
    } catch (e) {
      error = errorMessage(e);
      input?.focus();
    } finally {
      working = false;
    }
  }
</script>

<main class="narrow">
  <h1>Shiftbook is locked</h1>
  <p class="lead">
    {kind === 'password'
      ? 'Enter your password to open your work log.'
      : 'Enter the recovery key you saved when you set up Shiftbook.'}
  </p>

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
  </form>
</main>
