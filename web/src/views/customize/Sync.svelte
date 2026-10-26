<script lang="ts">
  // Settings → Phone and computer (round 15): swap changes through a sync file. One button makes
  // a locked file with everything on this device; the other brings a file from the other device
  // in, after showing what it holds. Shiftbook never goes online: the file travels any way the
  // user likes. The merge is the app's (crates/storage/src/sync.rs): newer edit wins, deletes
  // can be brought back, settings stay on each device, a safety backup is made first.
  import { api } from '../../lib/api';
  import { ApiError } from '../../lib/api/DataApi';
  import type { SecretKind, SyncReport } from '../../lib/api/DataApi';
  import { madeAt } from '../../lib/backups';
  import { toIsoDate } from '../../lib/dates';
  import { errorMessage } from '../../lib/messages';
  import { phone } from '../../lib/phone.svelte';
  import { attempt } from '../../lib/store.svelte';
  import { nothingNew, reportLines } from '../../lib/sync';
  import { toast } from '../../lib/toast.svelte';

  const other = $derived(phone.on ? 'computer' : 'phone');
  const here = $derived(phone.on ? 'phone' : 'computer');

  let last = $state<string | null>(null);
  let making = $state(false);

  // The file being brought in, and what it holds once checked.
  let fileName = $state('');
  let report = $state<SyncReport | null>(null);
  let needsSecret = $state(false);
  let secret = $state('');
  let kind = $state<SecretKind>('password');
  // The secret that opened the file, held only until it's brought in or Cancel.
  let usedSecret: { secret: string; kind: SecretKind } | null = null;
  let working = $state(false);
  let error = $state('');

  async function refresh() {
    last = (await attempt(() => api.lastSwap())) ?? null;
  }

  $effect(() => {
    void refresh();
  });

  async function make() {
    if (making) return;
    making = true;
    const name = await attempt(() => api.makeSyncFile(toIsoDate(new Date())));
    making = false;
    if (name) {
      toast(`Sync file saved as “${name}”. Open it in Shiftbook on your ${other}.`);
      await refresh();
    }
  }

  function reset() {
    fileName = '';
    report = null;
    needsSecret = false;
    secret = '';
    usedSecret = null;
    error = '';
    working = false;
  }

  async function pick() {
    const name = await attempt(() => api.pickSyncFile());
    if (!name) return;
    reset();
    fileName = name;
    await check();
  }

  async function check() {
    const typed = needsSecret ? { secret, kind } : null;
    secret = '';
    error = '';
    working = true;
    try {
      report = await api.checkSyncFile(typed?.secret, typed?.kind);
      usedSecret = typed;
      needsSecret = false;
    } catch (e) {
      if (e instanceof ApiError && e.code === 'backup_needs_secret') {
        needsSecret = true;
      } else {
        error = errorMessage(e);
      }
    } finally {
      working = false;
    }
  }

  async function bringIn() {
    if (!report || working) return;
    error = '';
    working = true;
    try {
      await api.bringInSyncFile(usedSecret?.secret, usedSecret?.kind);
      usedSecret = null;
      // Start again with the merged data everywhere.
      window.location.reload();
    } catch (e) {
      error = errorMessage(e);
      working = false;
    }
  }
</script>

<article class="paper cz-card">
  <h3>Phone and computer</h3>
  <p class="sub">{last ? `Last swapped ${madeAt(last)}.` : 'Not swapped yet.'}</p>
  <p class="sub">
    Move the file any way you like: a USB cable, a folder you already sync, or email it to yourself.
    It's locked like a backup, and Shiftbook itself never goes online.
  </p>
</article>

<article class="paper cz-card">
  <h3>Send my changes</h3>
  <p class="sub">
    Makes one locked file with everything on this {here}. Your {other} brings in only what's new or
    newer, so a missed file doesn't matter.
  </p>
  <div>
    <button class="btn primary big" type="button" onclick={make} disabled={making}>
      {making ? 'Making…' : 'Make a sync file'}
    </button>
  </div>
</article>

<article class="paper cz-card" class:checking={!!fileName}>
  <h3>Bring in changes</h3>
  {#if !fileName}
    <p class="sub">Pick a sync file made by your {other}. You'll see what it holds before anything changes.</p>
    <div><button class="btn big" type="button" onclick={pick}>Open a sync file</button></div>
  {:else if needsSecret}
    <p class="sub">
      “{fileName}” was locked by your {other}'s Shiftbook. Enter the password or recovery key you use
      there.
    </p>
    <div class="seg" role="radiogroup" aria-label="What to enter">
      <label><input type="radio" bind:group={kind} value="password" /> Password</label>
      <label><input type="radio" bind:group={kind} value="recovery_key" /> Recovery key</label>
    </div>
    <form
      class="cz-row"
      onsubmit={(e) => {
        e.preventDefault();
        if (secret) void check();
      }}
    >
      <input
        class="text-in cz-grow"
        type="password"
        bind:value={secret}
        autocomplete="off"
        aria-label={kind === 'password' ? `Password on your ${other}` : 'Recovery key'}
        placeholder={kind === 'password' ? 'Password' : 'Recovery key'}
        disabled={working}
      />
      <button class="btn" type="button" onclick={reset} disabled={working}>Cancel</button>
      <button class="btn primary" type="submit" disabled={!secret || working}>
        {working ? 'Opening…' : 'Open'}
      </button>
    </form>
  {:else if report}
    <p class="sub">From your {other} · made {madeAt(report.createdAt)}</p>
    {#if nothingNew(report)}
      <p>Nothing new: this {here} already has everything in it.</p>
      <div><button class="btn" type="button" onclick={reset}>Close</button></div>
    {:else}
      <dl class="counts">
        {#each reportLines(report) as line (line.label)}
          <dt>{line.label}</dt>
          <dd class="mono">{line.value}</dd>
        {/each}
      </dl>
      {#if report.clashCount > 0}
        <div class="clash">
          <p><b>Changed on both:</b></p>
          <ul>
            {#each report.clashes as c, i (i)}
              <li>
                “{c.title}” was edited here and on your {other}. The newer edit wins: {c.winner === 'there'
                  ? `your ${other}'s`
                  : `this ${here}'s`}, {madeAt(c.at)}.
              </li>
            {/each}
          </ul>
          {#if report.clashCount > report.clashes.length}
            <p class="sub">And {report.clashCount - report.clashes.length} more.</p>
          {/if}
          <p class="sub">The older edit stays in the safety backup.</p>
        </div>
      {/if}
      <p class="sub">A safety backup is made first, as with Restore. Settings stay as they are on each device.</p>
      <div class="cz-row acts">
        <button class="btn" type="button" onclick={reset} disabled={working}>Cancel</button>
        <button class="btn primary" type="button" onclick={bringIn} disabled={working}>
          {working ? 'Bringing them in…' : 'Bring them in'}
        </button>
      </div>
    {/if}
  {:else if working}
    <p class="sub">Checking the sync file…</p>
  {:else}
    <div><button class="btn" type="button" onclick={reset}>Close</button></div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</article>

<style>
  .big {
    min-height: 44px;
  }

  .checking {
    border: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
  }

  .counts {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px 16px;
    margin: 0;
  }

  .counts dt {
    color: var(--mut);
  }

  .counts dd {
    margin: 0;
    text-align: right;
    font-weight: 650;
  }

  .clash {
    border: 2px solid color-mix(in srgb, var(--warn) 50%, transparent);
    background: color-mix(in srgb, var(--warn) 8%, var(--paper));
    border-radius: 8px;
    padding: 4px 14px;
  }

  .clash ul {
    margin: 0;
    padding-left: 18px;
  }

  .seg {
    display: flex;
    gap: 16px;
  }

  .acts {
    justify-content: flex-end;
  }
</style>
