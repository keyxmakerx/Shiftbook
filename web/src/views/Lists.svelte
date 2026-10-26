<script lang="ts">
  // Lists (round 15): checklists you use again and again, like shopping, packing or "before we
  // go away". Tick things off as you go; Reset for next time unticks them all and keeps the
  // items. Paste several lines at once and each line becomes an item.
  import { onMount } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import type { ShopList } from '../lib/api';
  import { attempt } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  let lists = $state<ShopList[]>([]);
  let openId = $state<string | null>(null);
  let newTitle = $state('');
  let adding = $state('');
  let editing = $state<string | null>(null);
  let addEl: HTMLTextAreaElement | undefined = $state();

  const open = $derived(lists.find((l) => l.id === openId) ?? lists[0] ?? null);

  onMount(async () => {
    lists = (await attempt(() => api.lists())) ?? [];
  });

  function put(l: ShopList) {
    const i = lists.findIndex((x) => x.id === l.id);
    if (i >= 0) lists[i] = l;
    else lists.push(l);
  }

  async function create() {
    const title = newTitle.trim();
    if (!title) return;
    const l = await attempt(() => api.createList(title, ''));
    if (!l) return;
    put(l);
    openId = l.id;
    newTitle = '';
    setTimeout(() => addEl?.focus(), 0);
  }

  async function add() {
    if (!open || !adding.trim()) return;
    const id = open.id;
    const l = await attempt(() => api.addListItems(id, adding));
    if (!l) return;
    put(l);
    adding = '';
  }

  function addKey(e: KeyboardEvent) {
    // Enter adds; Shift+Enter makes a new line for several items.
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void add();
    }
  }

  async function tick(itemId: string, done: boolean) {
    const l = await attempt(() => api.listItemDone(itemId, done));
    if (l) put(l);
  }

  async function rename(itemId: string, text: string) {
    editing = null;
    if (!text.trim()) return;
    const l = await attempt(() => api.listItemText(itemId, text));
    if (l) put(l);
  }

  async function removeItem(itemId: string, text: string) {
    const l = await attempt(() => api.listItemDeleted(itemId, true));
    if (!l) return;
    put(l);
    toast(`Removed “${text}”.`, 'Undo', () => {
      void attempt(() => api.listItemDeleted(itemId, false)).then((back) => back && put(back));
    });
  }

  async function reset() {
    if (!open) return;
    const before = open.items.filter((i) => i.done).map((i) => i.id);
    const l = await attempt(() => api.resetList(open.id));
    if (!l) return;
    put(l);
    toast(`${l.title} is ready for next time.`, 'Undo', () => {
      void Promise.all(before.map((id) => attempt(() => api.listItemDone(id, true)))).then((all) => {
        const last = all.filter(Boolean).pop();
        if (last) put(last);
      });
    });
  }

  async function renameList(title: string) {
    if (!open || !title.trim() || title.trim() === open.title) return;
    const l = await attempt(() => api.renameList(open.id, title));
    if (l) put(l);
  }

  async function removeList() {
    if (!open) return;
    const l = open;
    const ok = await attempt(() => api.setListDeleted(l.id, true).then(() => true));
    if (!ok) return;
    lists = lists.filter((x) => x.id !== l.id);
    openId = null;
    toast(`Removed the list “${l.title}”.`, 'Undo', () => {
      void attempt(() => api.setListDeleted(l.id, false).then(() => true)).then((back) => {
        if (back) {
          lists.push(l);
          openId = l.id;
        }
      });
    });
  }
</script>

<div class="lists">
  <aside class="paper picker" aria-label="Your lists">
    <h1>Lists</h1>
    <ul>
      {#each lists as l (l.id)}
        <li>
          <button type="button" class="pick" aria-pressed={open?.id === l.id} onclick={() => (openId = l.id)}>
            <Icon name="list" size={15} />
            <span class="t">{l.title}</span>
            <span class="chip" class:all={l.total > 0 && l.done === l.total}>{l.done} of {l.total}</span>
          </button>
        </li>
      {/each}
    </ul>
    <form
      class="new"
      onsubmit={(e) => {
        e.preventDefault();
        void create();
      }}
    >
      <input class="text-in" bind:value={newTitle} placeholder="New list, e.g. Shopping" aria-label="New list name" maxlength="200" />
      <button class="btn" type="submit" disabled={!newTitle.trim()}><Icon name="plus" size={14} /> Make</button>
    </form>
  </aside>

  <section class="paper sheet" aria-label={open?.title ?? 'List'}>
    {#if open}
      <header>
        <input
          class="title"
          value={open.title}
          aria-label="List name"
          onchange={(e) => void renameList((e.currentTarget as HTMLInputElement).value)}
        />
        <span class="chip" class:all={open.total > 0 && open.done === open.total}>{open.done} of {open.total}</span>
      </header>
      <ul class="items">
        {#each open.items as i (i.id)}
          <li class:done={i.done}>
            <label class="tick">
              <input type="checkbox" checked={i.done} onchange={(e) => void tick(i.id, (e.currentTarget as HTMLInputElement).checked)} />
              <span class="visually-hidden">{i.done ? 'Done' : 'Not done'}: {i.text}</span>
            </label>
            {#if editing === i.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="text-in edit"
                value={i.text}
                autofocus
                aria-label="Item"
                onblur={(e) => void rename(i.id, (e.currentTarget as HTMLInputElement).value)}
                onkeydown={(e) => {
                  if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
                  if (e.key === 'Escape') editing = null;
                }}
              />
            {:else}
              <button type="button" class="txt" title="Change the words" onclick={() => (editing = i.id)}>{i.text}</button>
            {/if}
            <button type="button" class="ib" aria-label="Remove {i.text}" onclick={() => void removeItem(i.id, i.text)}><Icon name="x" size={14} /></button>
          </li>
        {:else}
          <li class="empty-note">Nothing on this list yet. Type below, or paste a list: each line becomes an item.</li>
        {/each}
      </ul>
      <div class="add">
        <textarea
          class="text-in"
          rows="1"
          bind:this={addEl}
          bind:value={adding}
          onkeydown={addKey}
          placeholder="Add an item (paste several lines to add them all)"
          aria-label="Add an item"
        ></textarea>
        <button type="button" class="btn" onclick={() => void add()} disabled={!adding.trim()}><Icon name="plus" size={14} /> Add</button>
      </div>
      <div class="foot">
        <button type="button" class="btn danger" onclick={() => void removeList()}><Icon name="trash" size={14} /> Remove list</button>
        <button type="button" class="btn primary" onclick={() => void reset()} disabled={open.done === 0}>
          <Icon name="undo" size={14} /> Reset for next time
        </button>
      </div>
      <p class="note">Reset unticks everything and keeps the items, ready for the next shop or trip.</p>
    {:else}
      <p class="empty-note">
        No lists yet. Make one on the left for the things you go through again and again: shopping, packing, or what to do before
        you leave the house.
      </p>
    {/if}
  </section>
</div>

<style>
  .lists {
    display: grid;
    grid-template-columns: minmax(220px, 300px) minmax(0, 620px);
    gap: 14px;
    align-items: start;
    justify-content: center;
  }

  @media (max-width: 760px) {
    .lists {
      grid-template-columns: 1fr;
    }
  }

  .picker,
  .sheet {
    padding: 16px 18px;
  }

  h1 {
    margin: 0 0 10px;
    font: 650 24px var(--display);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .pick {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    background: none;
    padding: 8px 8px;
    border-radius: 8px;
    cursor: pointer;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .pick[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    font-weight: 600;
  }

  .pick .t {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip {
    cursor: default;
  }

  .chip.all {
    --c: var(--ok);
  }

  .new {
    display: flex;
    gap: 6px;
    margin-top: 12px;
  }

  .new .text-in {
    flex: 1;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }

  .title {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    font: 650 22px var(--display);
    color: inherit;
    padding: 2px 4px;
    border-radius: 6px;
  }

  .title:focus {
    outline: 0;
    box-shadow: 0 0 0 2px var(--accent);
  }

  .items li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 4px;
    border-bottom: 1px solid var(--rule);
  }

  .tick input {
    width: 19px;
    height: 19px;
    accent-color: var(--ok);
    cursor: pointer;
  }

  .txt {
    flex: 1;
    border: 0;
    background: none;
    text-align: left;
    padding: 4px 0;
    font: inherit;
    font-size: 15px;
    color: inherit;
    cursor: text;
  }

  li.done .txt {
    text-decoration: line-through;
    color: var(--mut);
  }

  .edit {
    flex: 1;
  }

  .ib {
    border: 0;
    background: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: var(--mut);
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0.6;
  }

  .ib:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .add {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    align-items: flex-start;
  }

  .add textarea {
    flex: 1;
    resize: vertical;
    font: inherit;
    min-height: 38px;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-top: 14px;
  }

  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--mut);
  }
</style>
