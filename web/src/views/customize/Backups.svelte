<script lang="ts">
  // Customize → Backups: save a backup now, see the automatic ones, and restore one. Every
  // backup is encrypted; the app checks it and saves a copy of the current data before a
  // restore.
  import { api } from '../../lib/api';
  import { ApiError, PICKED_BACKUP } from '../../lib/api/DataApi';
  import type { BackupFile, BackupSummary, SecretKind } from '../../lib/api/DataApi';
  import { backupLabel, fileSize, holds, madeAt } from '../../lib/backups';
  import { toIsoDate } from '../../lib/dates';
  import { errorMessage } from '../../lib/messages';
  import { attempt } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';

  let files = $state<BackupFile[] | null>(null);
  let saving = $state(false);

  // The backup being restored: where it comes from, and what it holds once checked.
  let source = $state<string | null>(null);
  let sourceName = $state('');
  let summary = $state<BackupSummary | null>(null);
  let needsSecret = $state(false);
  let secret = $state('');
  let kind = $state<SecretKind>('password');
  // The secret that opened the backup, held only until the restore or Cancel.
  let usedSecret: { secret: string; kind: SecretKind } | null = null;
  let working = $state(false);
  let error = $state('');

  async function refresh() {
    files = (await attempt(() => api.listBackups())) ?? [];
  }

  $effect(() => {
    void refresh();
  });

  async function backupNow() {
    if (saving) return;
    saving = true;
    const name = await attempt(() => api.backupNow(toIsoDate(new Date())));
    saving = false;
    if (name) toast(`Backup saved as “${name}”.`);
  }

  function reset() {
    source = null;
    sourceName = '';
    summary = null;
    needsSecret = false;
    secret = '';
    usedSecret = null;
    error = '';
    working = false;
  }

  async function check(from: string, name: string) {
    reset();
    source = from;
    sourceName = name;
    await open();
  }

  async function pickFile() {
    const name = await attempt(() => api.pickBackupFile());
    if (name) await check(PICKED_BACKUP, name);
  }

  async function open() {
    if (!source) return;
    const typed = needsSecret ? { secret, kind } : null;
    secret = '';
    error = '';
    working = true;
    try {
      summary = await api.checkBackup(source, typed?.secret, typed?.kind);
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

  async function restore() {
    if (!source || !summary || working) return;
    error = '';
    working = true;
    try {
      await api.restoreBackup(source, usedSecret?.secret, usedSecret?.kind);
      usedSecret = null;
      // Start again with the restored data everywhere.
      window.location.reload();
    } catch (e) {
      error = errorMessage(e);
      working = false;
    }
  }
</script>

<article class="paper cz-card">
  <h3>Back up now</h3>
  <p class="sub">
    Save a backup file wherever you like, such as a USB stick. Keep one away from this computer in
    case it breaks or is lost.
  </p>
  <div><button class="btn primary" type="button" onclick={backupNow} disabled={saving}>
    {saving ? 'Saving…' : 'Back up now…'}
  </button></div>
</article>

{#if source}
  <article class="paper cz-card restore" aria-labelledby="restore-title">
    <h3 id="restore-title">Restore “{sourceName}”</h3>
    {#if needsSecret}
      <p class="sub">
        This backup was made with a different password, for example on another computer or before
        you deleted all data. Enter the password or recovery key from when it was made.
      </p>
      <div class="seg" role="radiogroup" aria-label="What to enter">
        <label><input type="radio" bind:group={kind} value="password" /> Password</label>
        <label><input type="radio" bind:group={kind} value="recovery_key" /> Recovery key</label>
      </div>
      <form
        class="cz-row"
        onsubmit={(e) => {
          e.preventDefault();
          if (secret) void open();
        }}
      >
        <input
          class="text-in cz-grow"
          type="password"
          bind:value={secret}
          autocomplete="off"
          aria-label={kind === 'password' ? 'Password from when the backup was made' : 'Recovery key'}
          placeholder={kind === 'password' ? 'Password' : 'Recovery key'}
          disabled={working}
        />
        <button class="btn" type="button" onclick={reset} disabled={working}>Cancel</button>
        <button class="btn primary" type="submit" disabled={!secret || working}>
          {working ? 'Opening…' : 'Open backup'}
        </button>
      </form>
    {:else if summary}
      <p>
        Made <b>{madeAt(summary.createdAt)}</b>. It holds {holds(summary)}.
      </p>
      <div class="warn">
        <p>
          Restoring replaces everything in Shiftbook now with this backup. Your data as it is now is
          saved first, so you can go back to it from the list below.
        </p>
      </div>
      <div class="cz-row">
        <button class="btn" type="button" onclick={reset} disabled={working}>Cancel</button>
        <button class="btn primary" type="button" onclick={restore} disabled={working}>
          {working ? 'Restoring…' : 'Restore this backup'}
        </button>
      </div>
    {:else if working}
      <p class="sub">Checking the backup…</p>
    {:else}
      <div><button class="btn" type="button" onclick={reset}>Close</button></div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </article>
{/if}

<article class="paper cz-card">
  <h3>Backups on this computer</h3>
  <p class="sub">
    Shiftbook saves a backup once a day when you unlock it and keeps the last 14. Like your data,
    they need your password to open.
  </p>
  {#if files === null}
    <p class="sub">Loading…</p>
  {:else if files.length === 0}
    <p class="empty">No backups yet. The first one is made the next time you unlock Shiftbook.</p>
  {:else}
    <ul class="list">
      {#each files as f (f.name)}
        <li>
          <span class="cz-grow">
            <b>{madeAt(f.createdAt)}</b>
            <span class="meta">{backupLabel(f.kind)} · {fileSize(f.bytes)}</span>
          </span>
          <button class="btn" type="button" onclick={() => check(f.name, madeAt(f.createdAt))} disabled={working}>
            Restore…
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <div><button class="btn" type="button" onclick={pickFile} disabled={working}>Restore from a file…</button></div>
</article>

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 8px;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
  }

  .list li + li {
    border-top: 1px solid var(--line);
  }

  .list b {
    display: block;
  }

  .meta,
  .empty {
    font-size: 13px;
    color: var(--mut);
  }

  .restore {
    border: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
  }

  .warn {
    border: 2px solid color-mix(in srgb, var(--bad) 45%, transparent);
    background: color-mix(in srgb, var(--bad) 7%, var(--paper));
    border-radius: 8px;
    padding: 4px 14px;
  }

  .seg {
    display: flex;
    gap: 16px;
  }
</style>
