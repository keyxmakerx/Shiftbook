<script lang="ts">
  // The Inbox: scraps saved from capture (Ctrl+Enter, or a line with no hours).
  import { api } from '../lib/api';
  import type { InboxItem } from '../lib/api';
  import { app, attempt, focusLog, refreshInbox } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';

  async function drop(item: InboxItem, logIt: boolean) {
    const ok = await attempt(async () => {
      await api.setInboxDeleted(item.id, true);
      return true;
    });
    if (!ok) return;
    await attempt(refreshInbox);
    if (logIt) {
      focusLog(item.text);
      return;
    }
    toast('Removed from the Inbox', 'Undo', () => {
      void attempt(() => api.setInboxDeleted(item.id, false)).then(() => attempt(refreshInbox));
    }, () => void attempt(() => api.setInboxDeleted(item.id, true)).then(() => attempt(refreshInbox)));
  }
</script>

<article class="paper card">
  <h3><span class="title"><Icon name="note" size={16} /> Inbox</span><span class="lbl">{app.inbox.length}</span></h3>
  {#each app.inbox as item (item.id)}
    <div class="scrap">
      <span class="text">{item.text}</span>
      <button type="button" onclick={() => drop(item, true)}>Log it</button>
      <button type="button" aria-label="Remove “{item.text}”" onclick={() => drop(item, false)}><Icon name="x" size={13} /></button>
    </div>
  {:else}
    <p class="empty-note">Nothing waiting. Ctrl+Enter in capture saves a half-thought here for later.</p>
  {/each}
</article>

<style>
  .card {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 3px solid #de9930;
  }

  h3 {
    margin: 0;
    font: 600 14px var(--ui);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .title {
    display: inline-flex;
    gap: 7px;
    align-items: center;
  }

  .scrap {
    padding: 7px 10px;
    border-radius: 6px;
    background: var(--paper-2);
    border: 1.5px dashed color-mix(in srgb, #dd5536 55%, transparent);
    font-size: 13px;
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .scrap button {
    border: 0;
    background: none;
    color: var(--accent);
    cursor: pointer;
    font-weight: 600;
    font-size: 12px;
    padding: 3px 5px;
    border-radius: 5px;
    display: inline-flex;
    align-items: center;
  }

  .scrap button:hover {
    background: color-mix(in srgb, var(--ink) 7%, transparent);
  }
</style>
