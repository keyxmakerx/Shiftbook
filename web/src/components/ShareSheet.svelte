<script lang="ts">
  // Phone (round 15, idea C): text shared into Shiftbook from mail or a ticket app. The ticket
  // numbers are found the same way capture finds them (crates/core/src/capture.rs) and you
  // pick where it goes: the Done tickets tray, one entry, or the Inbox as a scrap.
  import { api } from '../lib/api';
  import { launch, openCapture } from '../lib/launch.svelte';
  import { applyTray } from '../lib/trayActions';
  import { app, attempt, refreshInbox, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  type Where = 'tray' | 'entry' | 'scrap';

  /** The Inbox keeps one line of up to 2,000 characters (crates/core/src/validate.rs). */
  const SCRAP_CHARS = 2000;

  let tickets = $state<string[]>([]);
  let where = $state<Where>('scrap');

  $effect(() => {
    const text = launch.share;
    if (text === null) return;
    tickets = [];
    where = 'scrap';
    void api
      .parseCapture(text, app.today)
      .then((c) => {
        if (launch.share !== text) return;
        tickets = c.tickets;
        where = c.tickets.length ? 'tray' : 'scrap';
      })
      .catch(() => undefined);
  });

  function close() {
    launch.share = null;
  }

  async function add() {
    const text = launch.share ?? '';
    close();
    if (where === 'tray') {
      await applyTray(await attempt(() => api.trayAdd(app.today, tickets.join('\n'))));
      setView('desk');
      toast(tickets.length === 1 ? `${tickets[0]} is in today's tray` : `${tickets.length} tickets are in today's tray`);
    } else if (where === 'entry') {
      const firstLine = text.split('\n').find((l) => l.trim()) ?? '';
      setView('desk');
      openCapture(`15m ${tickets.length ? tickets.join(' ') : firstLine.slice(0, 80)}`);
    } else {
      const line = text.replace(/\s+/g, ' ').trim().slice(0, SCRAP_CHARS);
      const item = await attempt(() => api.addInbox(line));
      if (!item) return;
      await attempt(refreshInbox);
      toast('Saved to the Inbox', 'Undo', () => {
        void attempt(() => api.setInboxDeleted(item.id, true)).then(() => attempt(refreshInbox));
      });
    }
  }
</script>

{#if launch.share !== null}
  <div class="scrim" onclick={close} aria-hidden="true"></div>
  <div class="paper sheet" role="dialog" aria-modal="true" aria-labelledby="share-head">
    <h2 id="share-head">Add to Shiftbook</h2>
    <p class="quote">{launch.share.slice(0, 240)}{launch.share.length > 240 ? '…' : ''}</p>
    {#if tickets.length}
      <p class="sub">Found {tickets.length === 1 ? '1 ticket' : `${tickets.length} tickets`} in what you shared:</p>
      <div class="chips">
        {#each tickets as t (t)}<span class="chip mono">{t}</span>{/each}
      </div>
    {/if}
    <div class="choices" role="radiogroup" aria-label="Where it goes">
      {#if tickets.length}
        <label class="choice">
          <input type="radio" name="share-where" value="tray" bind:group={where} />
          <span><b>Put in today's Done tickets tray</b><small>15 min each, change it later</small></span>
        </label>
      {/if}
      <label class="choice">
        <input type="radio" name="share-where" value="entry" bind:group={where} />
        <span><b>Log as one entry</b><small>Opens capture so you can set the time</small></span>
      </label>
      <label class="choice">
        <input type="radio" name="share-where" value="scrap" bind:group={where} />
        <span><b>Save the text as a scrap</b><small>In the Inbox, for later</small></span>
      </label>
    </div>
    <div class="acts">
      <button class="btn" type="button" onclick={close}>Cancel</button>
      <button class="btn primary" type="button" onclick={() => void add()}>Add</button>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: color-mix(in srgb, var(--desk) 40%, transparent);
  }

  .sheet {
    position: fixed;
    z-index: 41;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 18px 18px 0 0;
    padding: 18px 16px calc(16px + env(safe-area-inset-bottom, 0px));
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 85vh;
    overflow-y: auto;
  }

  h2 {
    margin: 0;
    font: 650 20px var(--display);
  }

  .quote {
    margin: 0;
    padding: 8px 10px;
    border-left: 3px solid var(--rule);
    color: var(--mut);
    font-size: 13px;
    white-space: pre-line;
  }

  .sub {
    margin: 0;
    font-size: 13.5px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    border: 1px solid var(--rule);
    border-radius: 999px;
    padding: 3px 9px;
    font-size: 12.5px;
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .choice {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px;
    border: 1px solid var(--rule);
    border-radius: 10px;
    min-height: 44px;
  }

  .choice:has(input:checked) {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .choice small {
    display: block;
    color: var(--mut);
  }

  .acts {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .acts .btn {
    min-height: 44px;
    flex: 1;
  }
</style>
