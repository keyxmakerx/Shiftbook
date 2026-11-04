<script lang="ts">
  // Link chips under a notepad (round 18): each [[Page title]] written in it, one click away.
  // A page that doesn't exist yet is made, ready to write.
  import { api } from '../lib/api';
  import { attempt, openWiki } from '../lib/store.svelte';
  import { wikiLinks } from '../lib/wikiText';
  import Icon from './Icon.svelte';

  let { text }: { text: string | null | undefined } = $props();
  const titles = $derived(wikiLinks(text));

  async function go(title: string) {
    const id = await attempt(() => api.wikiPageByTitle(title));
    if (id) return openWiki(id);
    const made = await attempt(() => api.createNote({ kind: 'page', title, body: '', projectId: null, tags: '' }));
    if (made) openWiki(made.id, true);
  }
</script>

{#if titles.length}
  <div class="wl" aria-label="Wiki pages linked here">
    {#each titles as t (t)}
      <button type="button" title="Open “{t}” in the wiki" onclick={() => void go(t)}><Icon name="book" size={12} />{t}</button>
    {/each}
  </div>
{/if}

<style>
  .wl {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    color: var(--accent);
    border-radius: 999px;
    padding: 1px 8px;
    font: 600 12px var(--ui);
    cursor: pointer;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button:hover {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
</style>
