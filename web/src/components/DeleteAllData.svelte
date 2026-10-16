<script lang="ts">
  // Delete all data and start again. Used in Customize → Security and on the unlock
  // screen (for someone who lost both the password and the recovery key). The button
  // only turns on once the confirmation word is typed; the app checks it again.
  import { api } from '../lib/api';
  import { clearCopiedSecret } from '../lib/clipboard';
  import { confirmsDelete, DELETE_CONFIRM_WORD } from '../lib/deleteData';
  import { errorMessage } from '../lib/messages';

  let { locked = false, onCancel }: { locked?: boolean; onCancel: () => void } = $props();

  let typed = $state('');
  let working = $state(false);
  let error = $state('');
  let input: HTMLInputElement | undefined = $state();

  const canDelete = $derived(confirmsDelete(typed) && !working);

  $effect(() => {
    input?.focus();
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canDelete) return;
    error = '';
    working = true;
    try {
      await api.deleteAllData(typed);
      await clearCopiedSecret();
      // Back to the first-run screen with nothing left in memory.
      window.location.reload();
    } catch (e) {
      error = errorMessage(e);
      working = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && !working) {
      event.preventDefault();
      onCancel();
    }
  }
</script>

<section class="wipe" aria-labelledby="delete-all-title">
  <h2 id="delete-all-title">Delete all data</h2>
  <div class="warn">
    <p>
      This deletes everything Shiftbook keeps on this computer: your time log, tickets, projects,
      notes and settings, and your password and recovery key with them.
    </p>
    <p><strong>It can't be undone.</strong> There is no backup to bring it back.</p>
    {#if locked}
      <p>
        Use this only if you've lost both your password and your recovery key. Shiftbook will start
        fresh and ask you to choose a new password.
      </p>
    {:else}
      <p>Shiftbook will then start fresh and ask you to choose a new password.</p>
    {/if}
  </div>

  <form onsubmit={submit} class="form">
    <label for="delete-confirm">Type <b>{DELETE_CONFIRM_WORD}</b> to confirm</label>
    <input
      bind:this={input}
      id="delete-confirm"
      class="text-in"
      type="text"
      bind:value={typed}
      autocomplete="off"
      spellcheck="false"
      autocapitalize="characters"
      disabled={working}
      onkeydown={onKeydown}
    />
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button class="btn" type="button" onclick={onCancel} disabled={working}>Cancel</button>
      <button class="btn go" type="submit" disabled={!canDelete}>
        {working ? 'Deleting…' : 'Delete everything'}
      </button>
    </div>
  </form>
</section>

<style>
  .wipe {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h2 {
    margin: 0;
    font-size: 1.15rem;
    color: var(--bad);
  }

  .warn {
    border: 2px solid color-mix(in srgb, var(--bad) 55%, transparent);
    background: color-mix(in srgb, var(--bad) 8%, var(--paper));
    border-radius: 8px;
    padding: 10px 14px;
  }

  .warn p {
    margin: 6px 0;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    font-weight: 600;
  }

  label b {
    font-family: var(--mono);
    letter-spacing: 0.06em;
  }

  input {
    font-family: var(--mono);
    font-size: 1.05rem;
    letter-spacing: 0.06em;
    max-width: 16rem;
  }

  .buttons {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 4px;
  }

  .go {
    background: var(--bad);
    border-color: var(--bad);
    color: var(--accent-fg);
    font-weight: 650;
  }
</style>
