<script lang="ts">
  import { api } from '../lib/api';
  import { errorMessage } from '../lib/messages';
  import { charLength, MIN_PASSWORD_LENGTH } from '../lib/password';
  import StrengthMeter from '../components/StrengthMeter.svelte';

  let { onDone }: { onDone: (recoveryKey: string) => void } = $props();

  let password = $state('');
  let confirm = $state('');
  let show = $state(false);
  let working = $state(false);
  let error = $state('');
  let first: HTMLInputElement | undefined = $state();

  const longEnough = $derived(charLength(password) >= MIN_PASSWORD_LENGTH);
  const matches = $derived(password === confirm);
  const canSubmit = $derived(longEnough && matches && !working);

  $effect(() => {
    first?.focus();
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit) return;
    // Take the password out of the form straight away.
    const chosen = password;
    password = '';
    confirm = '';
    error = '';
    working = true;
    try {
      const key = await api.vaultSetup(chosen);
      onDone(key);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<main class="narrow">
  <h1>Welcome to Shiftbook</h1>
  <p class="lead">
    Choose a password to protect your work log. Everything Shiftbook saves on this computer is
    locked with it.
  </p>

  <form onsubmit={submit} class="stack">
    <div class="field">
      <label for="new-password">Password</label>
      <input
        bind:this={first}
        id="new-password"
        type={show ? 'text' : 'password'}
        bind:value={password}
        autocomplete="off"
        spellcheck="false"
        autocapitalize="off"
        aria-describedby="password-rules password-strength"
        disabled={working}
      />
      <p id="password-rules" class="help">
        At least {MIN_PASSWORD_LENGTH} characters. A few unrelated words with spaces work well.
      </p>
      <StrengthMeter {password} id="password-strength" />
    </div>

    <div class="field">
      <label for="confirm-password">Type it again</label>
      <input
        id="confirm-password"
        type={show ? 'text' : 'password'}
        bind:value={confirm}
        autocomplete="off"
        spellcheck="false"
        autocapitalize="off"
        aria-invalid={confirm.length > 0 && !matches}
        aria-describedby="confirm-help"
        disabled={working}
      />
      <p id="confirm-help" class="help" class:problem={confirm.length > 0 && !matches}>
        {confirm.length > 0 && !matches ? "The two passwords don't match yet." : ' '}
      </p>
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={show} />
      Show password
    </label>

    <div class="notice" role="note">
      <strong>Keep it safe.</strong> Next you'll get a recovery key. If you lose both your password
      and your recovery key, your data cannot be recovered — not even by the developer.
    </div>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <button type="submit" class="primary" disabled={!canSubmit}>
      {working ? 'Setting up… this takes a few seconds' : 'Create password'}
    </button>
  </form>
</main>
