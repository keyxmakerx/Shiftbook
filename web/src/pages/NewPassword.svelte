<script lang="ts">
  // After unlocking with the recovery key, ask for a new password straight away (owner's
  // decision, round 6). The recovery key keeps working afterwards.
  import { api } from '../lib/api';
  import { errorMessage } from '../lib/messages';
  import { charLength, MIN_PASSWORD_LENGTH } from '../lib/password';
  import StrengthMeter from '../components/StrengthMeter.svelte';

  let { onDone }: { onDone: () => void } = $props();

  let password = $state('');
  let confirm = $state('');
  let working = $state(false);
  let error = $state('');
  let first: HTMLInputElement | undefined = $state();

  const canSubmit = $derived(charLength(password) >= MIN_PASSWORD_LENGTH && password === confirm && !working);

  $effect(() => {
    first?.focus();
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit) return;
    const chosen = password;
    password = '';
    confirm = '';
    error = '';
    working = true;
    try {
      await api.setNewPassword(chosen);
      onDone();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<main class="narrow">
  <h1>Choose a new password</h1>
  <p class="lead">
    You opened Shiftbook with your recovery key. Pick a new password now so you can unlock
    normally next time. Your recovery key keeps working.
  </p>
  <form onsubmit={submit} class="stack">
    <div class="field">
      <label for="np-password">New password</label>
      <input
        bind:this={first}
        id="np-password"
        type="password"
        bind:value={password}
        autocomplete="off"
        spellcheck="false"
        autocapitalize="off"
        aria-describedby="np-strength"
        disabled={working}
      />
      <StrengthMeter {password} id="np-strength" />
    </div>
    <div class="field">
      <label for="np-confirm">Type it again</label>
      <input
        id="np-confirm"
        type="password"
        bind:value={confirm}
        autocomplete="off"
        spellcheck="false"
        autocapitalize="off"
        aria-invalid={confirm.length > 0 && password !== confirm}
        disabled={working}
      />
      {#if confirm.length > 0 && password !== confirm}
        <p class="help problem">The two passwords don't match yet.</p>
      {/if}
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <button type="submit" class="primary" disabled={!canSubmit}>
      {working ? 'Saving… this takes a few seconds' : 'Save new password'}
    </button>
  </form>
</main>
