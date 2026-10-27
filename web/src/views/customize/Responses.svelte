<script lang="ts">
  // Settings → Responses: messages you paste into tickets again and again. The Done tickets
  // tray has a button that lists them; picking one copies it. Nothing is matched to a ticket.
  // Each one here has a Copy button too, which works while copying is blocked (round 15).
  import { onMount } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api } from '../../lib/api';
  import { copyText } from '../../lib/protection.svelte';
  import type { ResponseInput } from '../../lib/api';
  import { attempt } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';

  const MAX = 30;

  type Row = ResponseInput & { key: number };
  let rows = $state<Row[]>([]);
  let saved = $state('');
  let loaded = $state(false);
  let next = 0;

  const snapshot = (list: ResponseInput[]) => JSON.stringify(list.map((r) => [r.name.trim(), r.text.trim()]));
  const dirty = $derived(loaded && snapshot(rows) !== saved);

  function take(list: ResponseInput[]) {
    rows = list.map((r) => ({ name: r.name, text: r.text, key: next++ }));
    saved = snapshot(rows);
  }

  onMount(async () => {
    const list = await attempt(() => api.listResponses());
    if (list) take(list);
    loaded = true;
  });

  function add() {
    if (rows.length >= MAX) return;
    rows.push({ name: '', text: '', key: next++ });
  }

  function move(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= rows.length) return;
    const [row] = rows.splice(i, 1);
    rows.splice(j, 0, row!);
  }

  async function copy(row: Row) {
    const name = row.name.trim() || 'the response';
    toast((await copyText(row.text.trim())) ? `Copied “${name}”` : "Couldn't copy here.");
  }

  async function save() {
    const list = await attempt(() => api.saveResponses(rows.map(({ name, text }) => ({ name, text }))));
    if (!list) return;
    take(list);
    toast('Saved your responses');
  }
</script>

<article class="paper cz-card">
  <h3>Responses</h3>
  <p class="sub">
    Messages you paste into tickets again and again. Copy one here, or use the <b>Responses</b> button on the
    Tickets card on the Desk; pick one and it's copied, ready to paste.
  </p>

  {#if loaded}
    {#each rows as row, i (row.key)}
      <div class="resp">
        <div class="head">
          <input
            class="text-in name"
            bind:value={row.name}
            maxlength="60"
            placeholder="Name in the menu, e.g. No reply from user"
            aria-label="Response {i + 1} name"
          />
          <button class="btn copy" type="button" disabled={!row.text.trim()} onclick={() => void copy(row)}>
            <Icon name="copy" size={14} /> Copy
          </button>
          <span class="tools">
            <button type="button" class="ib" aria-label="Move up" disabled={i === 0} onclick={() => move(i, -1)}>
              <Icon name="left" size={14} />
            </button>
            <button
              type="button"
              class="ib"
              aria-label="Move down"
              disabled={i === rows.length - 1}
              onclick={() => move(i, 1)}
            >
              <Icon name="right" size={14} />
            </button>
            <button type="button" class="ib" aria-label="Remove response {i + 1}" onclick={() => rows.splice(i, 1)}>
              <Icon name="trash" size={14} />
            </button>
          </span>
        </div>
        <textarea
          class="text-in"
          bind:value={row.text}
          rows="3"
          maxlength="4000"
          placeholder="The message that gets copied"
          aria-label="Response {i + 1} message"
        ></textarea>
      </div>
    {:else}
      <p class="empty-note">No responses yet. Add the ones you type most, like "Fixed and confirmed with the user".</p>
    {/each}

    <div class="actions">
      <button class="btn" type="button" onclick={add} disabled={rows.length >= MAX}>
        <Icon name="plus" size={14} /> Add a response
      </button>
      <span class="grow"></span>
      {#if dirty}<span class="unsaved">Not saved yet</span>{/if}
      <button class="btn primary" type="button" onclick={save} disabled={!dirty}>Save</button>
    </div>
  {/if}
</article>

<style>
  .resp {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 0;
    border-top: 1px solid var(--rule);
  }

  .head {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }

  textarea {
    resize: vertical;
    min-height: 64px;
  }

  .copy {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .tools {
    display: inline-flex;
    gap: 2px;
  }

  /* Up and down reuse the left/right arrows, turned. */
  .tools .ib:nth-child(1) :global(svg),
  .tools .ib:nth-child(2) :global(svg) {
    transform: rotate(90deg);
  }

  .ib {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    cursor: pointer;
  }

  .ib:hover:not(:disabled) {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--ink);
  }

  .ib:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding-top: 6px;
  }

  .grow {
    flex: 1;
  }

  .unsaved {
    color: var(--warn);
    font-size: 12.5px;
  }
</style>
