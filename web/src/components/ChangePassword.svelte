<script lang="ts">
  import { api } from '../lib/api';
  import { errorMessage } from '../lib/messages';
  import { charLength, MIN_PASSWORD_LENGTH } from '../lib/password';
  import StrengthMeter from './StrengthMeter.svelte';

  let { onClose }: { onClose: () => void } = $props();

  let current = $state('');
  let next = $state('');
  let confirm = $state('');
  let working = $state(false);
  let error = $state('');
  let done = $state(false);
  let first: HTMLInputElement | undefined = $state();

  const canSubmit = $derived(
    current.length > 0 &&
      charLength(next) >= MIN_PASSWORD_LENGTH &&
      next === confirm &&
      !working,
  );

  $effect(() => {
    first?.focus();
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit) return;
    const oldPassword = current;
    const newPassword = next;
    current = '';
    next = '';
    confirm = '';
    error = '';
    working = true;
    try {
      await api.changePassword(oldPassword, newPassword);
      done = true;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<section class="panel" aria-labelledby="change-password-title">
  <h2 id="change-password-title">Change password</h2>
  {#if done}
    <p role="status">Password changed. Use the new one next time you unlock.</p>
    <p>Your recovery key still works.</p>
    <button type="button" onclick={onClose}>Close</button>
  {:else}
    <form onsubmit={submit} class="stack">
      <div class="field">
        <label for="current-password">Current password</label>
        <input
          bind:this={first}
          id="current-password"
          type="password"
          bind:value={current}
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          disabled={working}
        />
      </div>
      <div class="field">
        <label for="next-password">New password</label>
        <input
          id="next-password"
          type="password"
          bind:value={next}
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          aria-describedby="next-strength"
          disabled={working}
        />
        <StrengthMeter password={next} id="next-strength" />
      </div>
      <div class="field">
        <label for="next-confirm">Type the new password again</label>
        <input
          id="next-confirm"
          type="password"
          bind:value={confirm}
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          aria-invalid={confirm.length > 0 && next !== confirm}
          disabled={working}
        />
        {#if confirm.length > 0 && next !== confirm}
          <p class="help problem">The new passwords don't match yet.</p>
        {/if}
      </div>
      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      <div class="row">
        <button type="submit" class="primary" disabled={!canSubmit}>
          {working ? 'Changing…' : 'Change password'}
        </button>
        <button type="button" onclick={onClose} disabled={working}>Cancel</button>
      </div>
    </form>
  {/if}
</section>
