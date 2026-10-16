<script lang="ts">
  // Customize → Security: change password, auto-lock time, a new recovery key and deleting
  // all data.
  import { api } from '../../lib/api';
  import { CLEAR_AFTER_MS, copySecret } from '../../lib/clipboard';
  import { errorMessage } from '../../lib/messages';
  import { recoveryGroups } from '../../lib/recoveryKey';
  import { app, attempt } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';
  import ChangePassword from '../../components/ChangePassword.svelte';
  import DeleteAllData from '../../components/DeleteAllData.svelte';

  let changing = $state(false);
  let minutes = $state(app.prefs?.autoLockMinutes ?? 15);
  let password = $state('');
  let working = $state(false);
  let keyError = $state('');
  // Held only until "I saved it" is pressed.
  let newKey = $state<string | null>(null);
  let copied = $state(false);
  let deleting = $state(false);

  async function saveMinutes() {
    const p = await attempt(() => api.savePref('security.auto_lock_minutes', String(minutes)));
    if (p) {
      app.prefs = p;
      toast(`Shiftbook locks after ${p.autoLockMinutes} minutes without activity.`);
    }
  }

  async function makeKey(event: SubmitEvent) {
    event.preventDefault();
    if (!password || working) return;
    const typed = password;
    password = '';
    keyError = '';
    working = true;
    try {
      newKey = await api.regenerateRecoveryKey(typed);
      copied = false;
    } catch (e) {
      keyError = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<article class="paper cz-card">
  <h3>Password</h3>
  {#if changing}
    <ChangePassword onClose={() => (changing = false)} />
  {:else}
    <div><button class="btn" type="button" onclick={() => (changing = true)}>Change password</button></div>
  {/if}
</article>

<article class="paper cz-card">
  <h3>Lock automatically</h3>
  <p class="sub">After this many minutes without keyboard or mouse use. Ctrl+L locks at once.</p>
  <form
    class="cz-row"
    onsubmit={(e) => {
      e.preventDefault();
      void saveMinutes();
    }}
  >
    <input class="text-in mins" type="number" min="1" max="240" bind:value={minutes} aria-label="Minutes before locking" />
    <span class="cz-grow">minutes</span>
    <button class="btn primary" type="submit">Save</button>
  </form>
</article>

<article class="paper cz-card">
  <h3>Recovery key</h3>
  {#if newKey}
    <p class="sub">Your new recovery key. The old one no longer works. Write it down and keep it away from this computer.</p>
    <ol class="key" aria-label="New recovery key">
      {#each recoveryGroups(newKey) as group, i (i)}
        <li><span class="num" aria-hidden="true">{i + 1}</span><code>{group}</code></li>
      {/each}
    </ol>
    <div class="cz-row">
      <button
        class="btn"
        type="button"
        onclick={async () => {
          copied = await copySecret(newKey ?? '');
        }}>Copy key</button
      >
      <span class="cz-grow note">
        {copied
          ? `Copied. The clipboard is emptied in ${CLEAR_AFTER_MS / 1000} seconds.`
          : 'Careful: clipboard history or sync tools can keep a copy.'}
      </span>
      <button class="btn primary" type="button" onclick={() => (newKey = null)}>I saved it</button>
    </div>
  {:else}
    <p class="sub">Make a new recovery key if the old one is lost. Your password is needed.</p>
    <form class="cz-row" onsubmit={makeKey}>
      <input
        class="text-in cz-grow"
        type="password"
        bind:value={password}
        autocomplete="off"
        placeholder="Your password"
        aria-label="Your password"
        disabled={working}
      />
      <button class="btn primary" type="submit" disabled={!password || working}>
        {working ? 'Making a new key…' : 'Make a new recovery key'}
      </button>
    </form>
    {#if keyError}<p class="error" role="alert">{keyError}</p>{/if}
  {/if}
</article>

<article class="paper cz-card">
  {#if deleting}
    <DeleteAllData onCancel={() => (deleting = false)} />
  {:else}
    <h3>Delete all data</h3>
    <p class="sub">Remove everything Shiftbook keeps on this computer and start again. This can't be undone.</p>
    <div><button class="btn danger" type="button" onclick={() => (deleting = true)}>Delete all data…</button></div>
  {/if}
</article>

<style>
  .mins {
    width: 90px;
  }

  .note {
    font-size: 12.5px;
    color: var(--mut);
  }

  .key {
    list-style: none;
    margin: 0;
    padding: 12px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(6.5rem, 1fr));
    gap: 0.4rem 1rem;
    border: 2px dashed var(--line-strong);
    border-radius: 8px;
  }

  .key li {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .num {
    color: var(--mut);
    font-size: 0.8rem;
    min-width: 1.4rem;
    text-align: right;
  }

  code {
    font-family: var(--mono);
    font-size: 1.15rem;
    letter-spacing: 0.08em;
  }
</style>
