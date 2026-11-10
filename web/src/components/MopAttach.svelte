<script lang="ts">
  // Attach a MOP to a ticket (round 18, card r18-mop): pick a template (the ones used lately
  // and the ticket's project's come first, and a search box narrows a long list), answer its
  // {{blanks}} once (the ticket and title fill themselves), and the MOP opens in a window.
  // Or copy a wiki page in as it is, or start blank.
  // Quicker (owner ask): a template with nothing left to type makes the MOP in one click, the
  // number keys 1-9 pick from the list, and Enter in the search box takes the top match.
  import { api } from '../lib/api';
  import type { MopTemplate, WikiRow } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { makeMop, mops } from '../lib/mops.svelte';
  import { attempt, openWiki } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const ask = $derived(mops.attach);
  let templates = $state<MopTemplate[]>([]);
  let pages = $state<WikiRow[]>([]);
  let find = $state('');
  let picked = $state<MopTemplate | null>(null);
  let blanks = $state<{ name: string; value: string; auto: boolean }[]>([]);
  let pageId = $state('');

  const needle = $derived(find.trim().toLowerCase());
  // The cursor starts in the first blank still to fill (the ticket fills itself).
  const firstEmpty = $derived(Math.max(0, blanks.findIndex((b) => !b.value)));
  const shown = $derived(templates.filter((t) => !needle || `${t.title} ${t.hint}`.toLowerCase().includes(needle)));

  $effect(() => {
    const a = ask;
    if (!a) return;
    picked = null;
    find = '';
    pageId = '';
    void (async () => {
      templates = (await attempt(() => api.mopTemplates(a.ticket))) ?? [];
      // From a template's page in the wiki: straight to its blanks.
      const chosen = a.templateId ? templates.find((t) => t.id === a.templateId) : undefined;
      if (chosen) void pick(chosen);
      const wiki = await attempt(() => api.wiki());
      pages = wiki ? [...wiki.spaces.flatMap((s) => s.pages), ...wiki.loose] : [];
    })();
  });

  async function pick(t: MopTemplate) {
    const a = ask;
    if (!a) return;
    const note = await attempt(() => api.getNote(t.id));
    if (!note) return;
    const names = (await attempt(() => api.mopBlanks(`${note.title}\n${note.body}`))) ?? [];
    const filled = names.map((name) => {
      const n = name.toLowerCase();
      const value = n === 'ticket' ? a.ticket : n === 'title' ? a.title : '';
      return { name, value, auto: value !== '' };
    });
    // Nothing left to type: no need for a second step.
    if (filled.every((b) => b.auto)) {
      await makeMop(a.ticket, { templateId: t.id, values: filled.map((b) => [b.name, b.value]) });
      return;
    }
    blanks = filled;
    picked = t;
  }

  /** 1-9 pick the template with that number, unless typing in a box. */
  function numberKey(e: KeyboardEvent) {
    if (picked || e.ctrlKey || e.metaKey || e.altKey) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest('input, select, textarea')) return;
    const n = Number(e.key);
    if (!Number.isInteger(n) || n < 1 || n > 9) return;
    const t = shown[n - 1];
    if (!t) return;
    e.preventDefault();
    void pick(t);
  }

  function close() {
    mops.attach = null;
  }
</script>

{#if ask}
  <div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
    <div
      class="paper dlg"
      role="dialog"
      aria-modal="true"
      aria-label="Attach a MOP to {ask.ticket}"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          close();
        } else numberKey(e);
      }}
    >
      <header>
        <h3>{picked ? picked.title : `Attach a MOP to ${ask.ticket}`}</h3>
        <button type="button" class="x" aria-label="Close" onclick={close}><Icon name="x" size={14} /></button>
      </header>

      {#if picked}
        <p class="sub">Each blank once; it's filled in everywhere it appears. Leave one empty to fill it in later.</p>
        <form
          onsubmit={(e) => {
            e.preventDefault();
            void makeMop(ask.ticket, { templateId: picked!.id, values: blanks.map((b) => [b.name, b.value]) });
          }}
        >
          {#each blanks as b, i (b.name)}
            <label class="blank">
              <span>{b.name}</span>
              {#if i === firstEmpty}
                <input class="text-in" class:auto={b.auto} bind:value={b.value} use:focusNow />
              {:else}
                <input class="text-in" class:auto={b.auto} bind:value={b.value} />
              {/if}
            </label>
          {:else}
            <p class="sub">This template has no blanks; you'll get a copy to fill in.</p>
          {/each}
          <div class="row">
            <button type="button" class="btn" onclick={() => (picked = null)}>Back</button>
            <span class="grow"></span>
            <button type="submit" class="btn primary">Make the MOP</button>
          </div>
        </form>
      {:else}
        {#if templates.length > 5}
          <input
            class="text-in"
            type="search"
            bind:value={find}
            placeholder="Find a template… (Enter takes the top one)"
            aria-label="Find a template"
            use:focusNow
            onkeydown={(e) => {
              if (e.key === 'Enter' && shown[0]) {
                e.preventDefault();
                void pick(shown[0]);
              }
            }}
          />
        {/if}
        <div class="list">
          {#each shown as t, i (t.id)}
            {#if i === 0 && templates.length <= 5}
              <button type="button" class="opt" onclick={() => void pick(t)} use:focusNow>
                <span class="num" aria-hidden="true">{i + 1}</span>
                <span class="txt"><b>{t.title}</b>{#if t.hint}<small>{t.hint}</small>{/if}</span>
                <span class="tag">Template</span>
              </button>
            {:else}
            <button type="button" class="opt" onclick={() => void pick(t)}>
              {#if i < 9}<span class="num" aria-hidden="true">{i + 1}</span>{:else}<Icon name="book" size={16} />{/if}
              <span class="txt"><b>{t.title}</b>{#if t.hint}<small>{t.hint}</small>{/if}</span>
              <span class="tag">Template</span>
            </button>
            {/if}
          {:else}
            <p class="sub">
              {templates.length ? 'No template matches.' : 'No MOP templates yet. Make one in the wiki under Templates, with {{blanks}} for what changes each time.'}
            </p>
          {/each}
        </div>
        <p class="or">or</p>
        <div class="other">
          {#if pages.length}
            <label class="page">
              <span>Copy a wiki page in</span>
              <select class="text-in" bind:value={pageId} aria-label="Wiki page to copy in">
                <option value="">Pick a page…</option>
                {#each pages as p (p.id)}<option value={p.id}>{'  '.repeat(p.depth)}{p.title}</option>{/each}
              </select>
              <button type="button" class="btn" disabled={!pageId} onclick={() => void makeMop(ask.ticket, { pageId })}>Use it</button>
            </label>
          {/if}
          <div class="row">
            <button type="button" class="btn" onclick={() => void makeMop(ask.ticket, { title: ask.title ? `${ask.ticket} · ${ask.title}` : '' })}>
              <Icon name="plus" size={14} /> Start a blank MOP
            </button>
            <button
              type="button"
              class="btn"
              onclick={() => {
                close();
                openWiki(null);
              }}
            >
              <Icon name="book" size={14} /> Templates in the wiki
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.25);
    display: grid;
    place-items: start center;
    padding-top: 10vh;
  }

  .dlg {
    width: min(460px, calc(100vw - 32px));
    max-height: 76vh;
    overflow: auto;
    padding: 14px 16px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h3 {
    margin: 0;
    font: 600 17px var(--display);
  }

  .x {
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--paper);
    text-align: left;
    cursor: pointer;
    color: var(--ink);
  }

  .opt:hover,
  .opt:focus-visible {
    border-color: var(--ok);
    background: color-mix(in srgb, var(--ok) 6%, var(--paper));
  }

  .num {
    width: 18px;
    height: 18px;
    flex: none;
    border-radius: 5px;
    border: 1px solid var(--rule);
    display: grid;
    place-items: center;
    font: 600 11px var(--ui);
    color: var(--ink-2);
  }

  .txt {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .txt small {
    color: var(--mut);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    font: 700 9.5px var(--ui);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ok);
    border: 1px solid var(--ok);
    border-radius: 4px;
    padding: 0 4px;
  }

  .or {
    margin: 2px 0;
    font-size: 12px;
    color: var(--mut);
  }

  .other {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .page {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font: 600 12.5px var(--ui);
    color: var(--ink-2);
  }

  .page select {
    flex: 1 1 160px;
    min-width: 0;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .grow {
    flex: 1;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .blank {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font: 600 12px var(--ui);
    color: var(--mut);
  }

  .auto {
    color: var(--ok);
    font-weight: 600;
  }
</style>
