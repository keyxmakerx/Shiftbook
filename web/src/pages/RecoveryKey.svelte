<script lang="ts">
  import { api } from '../lib/api';
  import { CLEAR_AFTER_MS, copySecret } from '../lib/clipboard';
  import { errorMessage } from '../lib/messages';
  import { groupMatches, pickCheckGroups, recoveryGroups } from '../lib/recoveryKey';

  let { recoveryKey, onDone }: { recoveryKey: string; onDone: () => void } = $props();

  const groups = $derived(recoveryGroups(recoveryKey));
  // Two random groups to retype, chosen once when the page opens.
  let checkAt: number[] = $state([]);
  $effect(() => {
    if (checkAt.length === 0 && groups.length > 0) checkAt = pickCheckGroups(groups.length, 2);
  });

  let saved = $state(false);
  let typed = $state(['', '']);
  let copied = $state<'no' | 'yes' | 'failed'>('no');
  let working = $state(false);
  let error = $state('');
  let checkInputs: HTMLInputElement[] = $state([]);

  // When the check boxes appear, move to the first one (focusing also brings it into view).
  $effect(() => {
    checkInputs[0]?.focus();
  });

  const allMatch = $derived(
    checkAt.length === 2 && checkAt.every((at, i) => groupMatches(groups[at] ?? '', typed[i] ?? '')),
  );
  const canContinue = $derived(saved && allMatch && !working);

  async function copy() {
    copied = (await copySecret(recoveryKey)) ? 'yes' : 'failed';
  }

  async function finish(event: SubmitEvent) {
    event.preventDefault();
    if (!canContinue) return;
    working = true;
    error = '';
    try {
      await api.vaultConfirmRecoverySaved();
      typed = ['', ''];
      onDone();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<main class="narrow">
  <h1>Your recovery key</h1>
  <p class="lead">
    If you ever forget your password, this key is the only other way to open your data. Write it
    down or print it, and keep it somewhere safe away from this computer.
    <strong>It will not be shown again.</strong>
  </p>

  <ol class="key" aria-label="Recovery key, {groups.length} groups of 4 characters">
    {#each groups as group, i (i)}
      <li><span class="num" aria-hidden="true">{i + 1}</span><code>{group}</code></li>
    {/each}
  </ol>

  <div class="copy-row">
    <button type="button" onclick={copy}>Copy key</button>
    <p class="help">
      {#if copied === 'yes'}
        Copied. Shiftbook will empty the clipboard in {CLEAR_AFTER_MS / 1000} seconds.
      {:else if copied === 'failed'}
        Copying didn't work here. Please write the key down instead.
      {:else}
        Careful: clipboard history or sync tools can keep a copy. Writing it down is safer.
      {/if}
    </p>
  </div>

  <form onsubmit={finish} class="stack">
    <label class="check">
      <input type="checkbox" bind:checked={saved} />
      I saved my recovery key
    </label>

    {#if saved && checkAt.length === 2}
      <fieldset>
        <legend>Quick check: type two groups from your copy</legend>
        <div class="checks">
          {#each checkAt as at, i (at)}
            <div class="field short">
              <label for="check-{i}">Group {at + 1}</label>
              <input
                bind:this={checkInputs[i]}
                id="check-{i}"
                class="mono"
                bind:value={typed[i]}
                maxlength="9"
                autocomplete="off"
                spellcheck="false"
                autocapitalize="characters"
              />
            </div>
          {/each}
        </div>
        {#if typed.every((t) => t.length >= 4) && !allMatch}
          <p class="help problem">That doesn't match. Check your copy of the key.</p>
        {/if}
      </fieldset>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <button type="submit" class="primary" disabled={!canContinue}>Continue</button>
  </form>
</main>

<style>
  .key {
    list-style: none;
    margin: 0 0 10px;
    padding: 10px 12px;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px 8px;
    background: var(--paper-2);
    border-radius: 8px;
  }

  @media (max-width: 460px) {
    .key {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  .key li {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 4px;
    padding: 4px 0;
  }

  .num {
    color: var(--mut);
    font: 500 10px var(--mono);
    min-width: 1.2rem;
    text-align: right;
  }

  code {
    font: 600 14.5px var(--mono);
    letter-spacing: 0.06em;
  }

  .copy-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }

  .copy-row .help {
    flex: 1 1 14rem;
    margin: 0;
  }

  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 14px;
  }
</style>
