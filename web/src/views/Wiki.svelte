<script lang="ts">
  // The wiki (round 18, card r18-wiki): pages you write once and find again. On the left, the
  // spaces with their pages as a tree, pages in no space, and the MOP templates. In the middle,
  // the open page: read it (headings, lists, code, [[links]] you can click) or edit it (it
  // saves as you type, and keeps earlier versions). On the right, what links to it and the
  // tickets whose notepad mentions it. Rules (links, the tree, versions) are in
  // crates/core/src/wiki.rs.
  import { onDestroy, untrack } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import type { Note, NoteVersion, WikiList, WikiPage, WikiRow } from '../lib/api';
  import { focusNow } from '../lib/focus';
  import { openMenu, type MenuSpec } from '../lib/menu.svelte';
  import { app, attempt } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { openWindow } from '../lib/windows.svelte';
  import { blocks, type Part } from '../lib/wikiText';

  const SAVE_AFTER_MS = 700;

  let list = $state<WikiList>({ spaces: [], loose: [], templates: [] });
  let page = $state<WikiPage | null>(null);
  let editing = $state(false);
  let title = $state('');
  let body = $state('');
  let find = $state('');
  let timer: ReturnType<typeof setTimeout> | null = null;
  let versions = $state<NoteVersion[] | null>(null);
  let shownVersion = $state<NoteVersion | null>(null);
  let naming = $state<{ id: string | null; name: string } | null>(null);
  let moving = $state<{ space: string; parent: string } | null>(null);

  const read = $derived(page ? blocks(page.note.body) : []);
  const needle = $derived(find.trim().toLowerCase());
  const match = (r: WikiRow) => !needle || r.title.toLowerCase().includes(needle);
  const isTemplate = $derived(!!page?.note.template);

  async function loadList() {
    list = (await attempt(() => api.wiki())) ?? list;
  }

  async function open(id: string, edit = false) {
    await flush();
    const p = await attempt(() => api.wikiPage(id));
    if (!p) return;
    page = p;
    title = p.note.title;
    body = p.note.body;
    editing = edit;
    versions = null;
    shownVersion = null;
    moving = null;
  }

  // Open what `openWiki` asked for; otherwise the first page there is.
  $effect(() => {
    const ask = app.wikiOpen;
    untrack(() => {
      void (async () => {
        await loadList();
        if (ask.id) await open(ask.id, ask.edit);
        else if (!page) {
          const first = list.spaces.flatMap((s) => s.pages)[0] ?? list.loose[0] ?? list.templates[0];
          if (first) await open(first.id);
        }
      })();
    });
  });

  // Changes made elsewhere (a note linking here, a ticket's notepad) show up here too.
  $effect(() => {
    void app.changed;
    untrack(() => {
      void loadList();
      if (page && !editing) void attempt(() => api.wikiPage(page!.note.id)).then((p) => p && (page = p));
    });
  });

  function typed() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void flush(), SAVE_AFTER_MS);
  }

  /** Save what's typed, if it changed. */
  async function flush() {
    if (timer) clearTimeout(timer);
    timer = null;
    const p = page;
    if (!p || (title === p.note.title && body === p.note.body)) return;
    const n = p.note;
    const saved = await attempt(() =>
      api.updateNote(n.id, { kind: n.kind, title, body, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (!saved || page?.note.id !== n.id) return;
    page = { ...page, note: saved };
    if (saved.title !== n.title) void loadList();
  }

  onDestroy(() => void flush());

  async function done() {
    await flush();
    editing = false;
    if (page) await open(page.note.id);
  }

  /** A new page in a space (null: no space), under a page (or at the top). */
  async function newPage(spaceId: string | null, parentId: string | null = null) {
    const made = await attempt(() => api.createNote({ kind: 'page', title: 'New page', body: '', projectId: null, tags: '' }));
    if (!made) return;
    if (spaceId || parentId) await attempt(() => api.wikiPlace(made.id, spaceId, parentId, null));
    await loadList();
    await open(made.id, true);
  }

  async function newTemplate() {
    const made = await attempt(() =>
      api.createNote({
        kind: 'mop',
        title: 'New template',
        body: '# {{title}} at {{site}}\n\n## Before\n1. \n\n## Steps\n1. \n\n## Check it worked\n1. \n\n## Back out\n1. \n',
        projectId: null,
        tags: '',
        template: true,
      }),
    );
    if (!made) return;
    await loadList();
    await open(made.id, true);
  }

  /** Follow a [[link]]: open the page, or offer to make it. */
  async function follow(linkTitle: string) {
    const id = await attempt(() => api.wikiPageByTitle(linkTitle));
    if (id) return open(id);
    const made = await attempt(() => api.createNote({ kind: 'page', title: linkTitle, body: '', projectId: null, tags: '' }));
    if (!made) return;
    if (page?.spaceId) await attempt(() => api.wikiPlace(made.id, page!.spaceId, null, null));
    await loadList();
    await open(made.id, true);
    toast(`Made “${linkTitle}”. Write it, or Undo to leave it unwritten.`, 'Undo', () => {
      void attempt(() => api.setNoteDeleted(made.id, true)).then(async () => {
        await loadList();
        page = null;
      });
    });
  }

  function openLinker(l: { id: string; kind: string }) {
    if (l.kind === 'page') void open(l.id);
    else openWindow('note', l.id);
  }

  // ----- spaces -----
  async function saveSpace() {
    if (!naming || !naming.name.trim()) return;
    const n = naming;
    const id = await attempt(() => api.wikiSpaceSave(n.id, n.name, null));
    if (!id) return;
    naming = null;
    await loadList();
  }

  async function exportSpace(spaceId: string | null, name: string) {
    toast(`The files won't be locked: anyone who can open the folder can read them.`);
    const folder = await attempt(() => api.exportWikiSpace(spaceId, name));
    if (folder) toast(`Written to the folder “${folder}”.`);
  }

  function spaceMenu(e: MouseEvent, id: string, name: string) {
    const spec: MenuSpec = {
      sections: [
        {
          head: name,
          items: [
            { label: 'New page here', icon: 'plus', act: () => void newPage(id) },
            { label: 'Rename', icon: 'edit', act: () => (naming = { id, name }) },
            { label: 'Write out as Markdown files…', icon: 'save', act: () => void exportSpace(id, name) },
            {
              label: 'Remove the space (pages stay)',
              icon: 'trash',
              danger: true,
              act: () =>
                void attempt(() => api.wikiSpaceDelete(id)).then(async () => {
                  await loadList();
                  toast(`Removed ${name}. Its pages are under Not in a space.`);
                }),
            },
          ],
        },
      ],
    };
    openMenu(e.currentTarget as HTMLElement, spec);
  }

  // ----- the open page -----
  function siblings(): WikiRow[] {
    if (!page) return [];
    const rows = page.spaceId ? (list.spaces.find((s) => s.id === page!.spaceId)?.pages ?? []) : list.loose;
    const parent = page.trail.at(-1)?.id ?? null;
    // Rows at the page's depth sharing its parent: walk the tree rows keeping the last parent per depth.
    const out: WikiRow[] = [];
    const stack: string[] = [];
    for (const r of rows) {
      stack.length = r.depth;
      const p = stack[r.depth - 1] ?? null;
      if (p === parent) out.push(r);
      stack[r.depth] = r.id;
    }
    return out;
  }

  async function nudge(step: -1 | 1) {
    if (!page) return;
    const sibs = siblings();
    const at = sibs.findIndex((r) => r.id === page!.note.id);
    const to = at + step;
    if (at < 0 || to < 0 || to >= sibs.length) return;
    [sibs[at], sibs[to]] = [sibs[to]!, sibs[at]!];
    const parent = page.trail.at(-1)?.id ?? null;
    for (const [i, r] of sibs.entries()) await attempt(() => api.wikiPlace(r.id, page!.spaceId, parent, i));
    await loadList();
  }

  async function move() {
    if (!page || !moving) return;
    const m = moving;
    const ok = await attempt(() =>
      api.wikiPlace(page!.note.id, m.space || null, m.parent || null, null).then(() => true),
    );
    if (!ok) return;
    moving = null;
    await loadList();
    await open(page.note.id);
  }

  /** Pages a page can go under in a space: not itself or anything under it. */
  function under(spaceId: string): WikiRow[] {
    if (!page) return [];
    const rows = spaceId ? (list.spaces.find((s) => s.id === spaceId)?.pages ?? []) : list.loose;
    const out: WikiRow[] = [];
    let skipBelow: number | null = null;
    for (const r of rows) {
      if (skipBelow !== null && r.depth > skipBelow) continue;
      skipBelow = null;
      if (r.id === page.note.id) {
        skipBelow = r.depth;
        continue;
      }
      out.push(r);
    }
    return out;
  }

  async function showVersions() {
    if (!page) return;
    await flush();
    versions = (await attempt(() => api.noteVersions(page!.note.id))) ?? [];
    shownVersion = versions[0] ?? null;
  }

  async function bringBack(v: NoteVersion) {
    if (!page) return;
    const n: Note = page.note;
    const saved = await attempt(() =>
      api.updateNote(n.id, { kind: n.kind, title: v.title, body: v.body, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (!saved) return;
    versions = null;
    await open(n.id);
    await loadList();
    toast('Brought back that version. What it said before is kept as a version too.');
  }

  function pageMenu(e: MouseEvent) {
    if (!page) return;
    const p = page;
    const spec: MenuSpec = {
      sections: [
        {
          head: p.note.title,
          items: [
            ...(isTemplate
              ? []
              : [
                  { label: 'New page under this one', icon: 'plus' as const, act: () => void newPage(p.spaceId, p.note.id) },
                  { label: 'Move to…', icon: 'folder' as const, act: () => (moving = { space: p.spaceId ?? '', parent: p.trail.at(-1)?.id ?? '' }) },
                  { label: 'Move up', icon: 'up' as const, act: () => void nudge(-1) },
                  { label: 'Move down', icon: 'down' as const, act: () => void nudge(1) },
                ]),
            { label: `Earlier versions (${p.versions})`, icon: 'restore', act: () => void showVersions() },
            { label: 'Open in a window', icon: 'expand', act: () => openWindow('note', p.note.id) },
            {
              label: 'Delete this page',
              icon: 'trash',
              danger: true,
              act: () =>
                void attempt(() => api.setNoteDeleted(p.note.id, true)).then(async () => {
                  page = null;
                  await loadList();
                  toast(`Deleted “${p.note.title}”`, 'Undo', () => {
                    void attempt(() => api.setNoteDeleted(p.note.id, false)).then(async () => {
                      await loadList();
                      await open(p.note.id);
                    });
                  });
                }),
            },
          ],
        },
      ],
    };
    openMenu(e.currentTarget as HTMLElement, spec);
  }

  function when(ts: string): string {
    const d = new Date(ts);
    return Number.isNaN(d.getTime())
      ? ''
      : d.toLocaleString([], { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
  }

  const KIND_WORD: Record<string, string> = { jot: 'Jot', daily: 'Daily note', meeting: 'Meeting note', page: 'Page', mop: 'MOP' };
</script>

{#snippet line(ps: Part[])}
  {#each ps as p, i (i)}
    {#if p.kind === 'link'}
      <button type="button" class="wlink" onclick={() => void follow(p.title)}>{p.title}</button>
    {:else if p.kind === 'blank'}
      <span class="blank">{'{{'}{p.name}{'}}'}</span>
    {:else if p.kind === 'code'}
      <code>{p.text}</code>
    {:else}{p.text}{/if}
  {/each}
{/snippet}

{#snippet rows(list: WikiRow[])}
  <ul class="tree">
    {#each list.filter(match) as r (r.id)}
      <li>
        <button
          type="button"
          class:on={page?.note.id === r.id}
          style:padding-left="{8 + (needle ? 0 : r.depth) * 14}px"
          onclick={() => void open(r.id)}
        >
          {r.title}
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<section class="wiki paper" aria-label="Wiki">
  <aside class="side" aria-label="Pages">
    <input class="text-in search" type="search" bind:value={find} placeholder="Find a page…" aria-label="Find a page" />

    {#each list.spaces as sp (sp.id)}
      <div class="head">
        <span class="name"><i class="dot" style:background={sp.color ?? 'var(--accent)'}></i>{sp.name}</span>
        <span class="tools">
          <button type="button" class="mini" title="New page in {sp.name}" aria-label="New page in {sp.name}" onclick={() => void newPage(sp.id)}>
            <Icon name="plus" size={13} />
          </button>
          <button type="button" class="mini" title="More for {sp.name}" aria-label="More for {sp.name}" onclick={(e) => spaceMenu(e, sp.id, sp.name)}>
            <Icon name="more" size={13} />
          </button>
        </span>
      </div>
      {#if sp.pages.length}
        {@render rows(sp.pages)}
      {:else}
        <p class="none">No pages yet.</p>
      {/if}
    {/each}

    {#if list.loose.length}
      <div class="head">
        <span class="name">Not in a space</span>
        <span class="tools">
          <button type="button" class="mini" title="Write these out as Markdown files" aria-label="Write out pages in no space" onclick={() => void exportSpace(null, 'Wiki pages')}>
            <Icon name="save" size={13} />
          </button>
        </span>
      </div>
      {@render rows(list.loose)}
    {/if}

    <div class="head">
      <span class="name">Templates</span>
      <span class="tools">
        <button type="button" class="mini" title="New MOP template" aria-label="New MOP template" onclick={() => void newTemplate()}>
          <Icon name="plus" size={13} />
        </button>
      </span>
    </div>
    {#if list.templates.length}
      {@render rows(list.templates)}
    {:else}
      <p class="none">MOP templates you make show here.</p>
    {/if}

    {#if naming}
      <form
        class="naming"
        onsubmit={(e) => {
          e.preventDefault();
          void saveSpace();
        }}
      >
        <input class="text-in" bind:value={naming.name} use:focusNow placeholder="e.g. Network" aria-label="Space name" />
        <button type="submit" class="btn primary slim">{naming.id ? 'Rename' : 'Add'}</button>
        <button type="button" class="btn slim" onclick={() => (naming = null)}>Cancel</button>
      </form>
    {:else}
      <div class="row">
        <button type="button" class="btn slim" onclick={() => (naming = { id: null, name: '' })}><Icon name="plus" size={13} /> New space</button>
        {#if !list.spaces.length && !list.loose.length}
          <button type="button" class="btn slim" onclick={() => void newPage(null)}><Icon name="plus" size={13} /> New page</button>
        {/if}
      </div>
    {/if}
  </aside>

  <article class="main">
    {#if page}
      <div class="crumbs">
        {#if page.spaceName}<span>{page.spaceName}</span>›{:else if isTemplate}<span>Templates</span>›{/if}
        {#each page.trail as t (t.id)}
          <button type="button" class="crumb" onclick={() => void open(t.id)}>{t.title}</button>›
        {/each}
        <span class="here">{page.note.title}</span>
        <span class="grow"></span>
        {#if editing}
          <button type="button" class="btn primary slim" onclick={() => void done()}><Icon name="check" size={13} /> Done</button>
        {:else}
          <button type="button" class="btn slim" onclick={() => (editing = true)}><Icon name="edit" size={13} /> Edit</button>
        {/if}
        <button type="button" class="btn slim" aria-label="More for this page" onclick={pageMenu}><Icon name="more" size={13} /></button>
      </div>

      {#if moving}
        <form
          class="moving"
          onsubmit={(e) => {
            e.preventDefault();
            void move();
          }}
        >
          <label>
            Space
            <select class="text-in" bind:value={moving.space} onchange={() => moving && (moving.parent = '')}>
              <option value="">Not in a space</option>
              {#each list.spaces as sp (sp.id)}<option value={sp.id}>{sp.name}</option>{/each}
            </select>
          </label>
          <label>
            Under
            <select class="text-in" bind:value={moving.parent}>
              <option value="">At the top</option>
              {#each under(moving.space) as r (r.id)}<option value={r.id}>{'  '.repeat(r.depth)}{r.title}</option>{/each}
            </select>
          </label>
          <button type="submit" class="btn primary slim">Move</button>
          <button type="button" class="btn slim" onclick={() => (moving = null)}>Cancel</button>
        </form>
      {/if}

      {#if editing}
        <input class="title-in" bind:value={title} oninput={typed} aria-label="Page title" />
        <p class="meta">
          Saves as you type. Type <code>[[</code> and a page's title to link it{isTemplate ? ', and {{name}} for a blank to fill in each time' : ''}.
        </p>
        <textarea class="body-in" bind:value={body} oninput={typed} onblur={() => void flush()} use:focusNow aria-label="Page text" spellcheck="true"></textarea>
      {:else}
        <h1>
          {page.note.title}
          {#if isTemplate}<span class="badge">Template</span>{/if}
        </h1>
        <p class="meta">
          Edited {when(page.note.updatedAt)}{#if page.versions}{' · '}<button type="button" class="linkish" onclick={() => void showVersions()}
              >{page.versions} earlier {page.versions === 1 ? 'version' : 'versions'}</button
            >{/if}{#if page.spaceName}{' · in '}<b>{page.spaceName}</b>{/if}
        </p>
        <div class="read">
          {#each read as b, i (i)}
            {#if b.kind === 'heading'}
              {#if b.level === 1}<h2>{@render line(b.parts)}</h2>{:else if b.level === 2}<h3>{@render line(b.parts)}</h3>{:else}<h4>{@render line(b.parts)}</h4>{/if}
            {:else if b.kind === 'item'}
              <p class="item" style:padding-left="{18 + b.depth * 18}px"><span class="mk">{b.marker}</span>{@render line(b.parts)}</p>
            {:else if b.kind === 'code'}
              <pre>{b.text}</pre>
            {:else if b.kind === 'para'}
              <p>{@render line(b.parts)}</p>
            {:else}
              <div class="gap"></div>
            {/if}
          {:else}
            <p class="empty">Nothing written yet. Press Edit to start.</p>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="welcome">
        <h1>Wiki</h1>
        <p>Pages you write once and find again: how-tos, fixes and reference, in spaces like Network or Servers. Link pages by typing <code>[[</code> and a page's title.</p>
        <div class="row">
          <button type="button" class="btn primary" onclick={() => (naming = { id: null, name: '' })}><Icon name="plus" size={14} /> New space</button>
          <button type="button" class="btn" onclick={() => void newPage(null)}><Icon name="plus" size={14} /> New page</button>
        </div>
      </div>
    {/if}
  </article>

  <aside class="right" aria-label="About this page">
    {#if versions && page}
      <h5>Earlier versions</h5>
      <div class="box versions">
        {#each versions as v (v.id)}
          <button type="button" class:on={shownVersion?.id === v.id} onclick={() => (shownVersion = v)}>{when(v.at)}</button>
        {:else}
          <p class="none">None yet. One is kept when you change a page, at most every 10 minutes while typing.</p>
        {/each}
      </div>
      {#if shownVersion}
        <div class="box">
          <b>{shownVersion.title}</b>
          <pre class="peek">{shownVersion.body}</pre>
          <div class="row">
            <button type="button" class="btn primary slim" onclick={() => shownVersion && void bringBack(shownVersion)}>Bring this back</button>
            <button type="button" class="btn slim" onclick={() => (versions = null)}>Close</button>
          </div>
        </div>
      {/if}
    {:else if page}
      {#if page.children.length}
        <h5>Pages under this one</h5>
        <div class="box">
          {#each page.children as c (c.id)}<button type="button" class="linkish row-btn" onclick={() => void open(c.id)}>{c.title}</button>{/each}
        </div>
      {/if}
      <h5>Linked from</h5>
      <div class="box">
        {#each page.linkedFrom as l (l.id)}
          <button type="button" class="linkish row-btn" onclick={() => openLinker(l)}>{l.title}<small>{KIND_WORD[l.kind] ?? ''}</small></button>
        {:else}
          <p class="none">Nothing links here yet. Type [[{page.note.title}]] in a note or a ticket's notepad.</p>
        {/each}
      </div>
      <h5>On tickets</h5>
      <div class="box">
        {#each page.onTickets as t (t.number)}
          <div class="tk"><span class="num">{t.number}</span><span class="st" class:done={t.done}>{t.done ? 'Done' : 'Open'}</span></div>
        {:else}
          <p class="none">No ticket's notepad mentions it yet.</p>
        {/each}
      </div>
      <h5>Who can see it</h5>
      <div class="box">
        <p class="none lockline"><span class="ic"><Icon name="lock" size={12} /></span>Only you, in your locked file. Sharing a space with a team comes with the hosted version.</p>
      </div>
    {/if}
  </aside>
</section>

<style>
  .wiki {
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr) 260px;
    height: 100%;
    min-height: 0;
    border-radius: 12px;
    overflow: hidden;
  }

  .side {
    border-right: 1px solid var(--rule);
    padding: 10px;
    overflow: auto;
    background: color-mix(in srgb, var(--ink) 3%, var(--paper));
  }

  .search {
    width: 100%;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 14px 2px 4px;
  }

  .head .name {
    font: 700 10.5px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 3px;
  }

  .tools {
    display: flex;
    gap: 2px;
  }

  .mini {
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .mini:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .tree button {
    width: 100%;
    text-align: left;
    border: 0;
    background: none;
    border-radius: 6px;
    padding: 4px 8px;
    font: 13.5px var(--ui);
    color: var(--ink);
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tree button:hover {
    background: color-mix(in srgb, var(--ink) 6%, transparent);
  }

  .tree button.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    font-weight: 600;
  }

  .none {
    margin: 2px 4px 6px;
    font-size: 12.5px;
    color: var(--mut);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }

  .naming {
    display: flex;
    gap: 6px;
    margin-top: 12px;
    flex-wrap: wrap;
  }

  .naming .text-in {
    flex: 1 1 120px;
    min-width: 0;
  }

  .main {
    padding: 14px 24px;
    overflow: auto;
    min-width: 0;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--mut);
    flex-wrap: wrap;
  }

  .crumb {
    border: 0;
    background: none;
    padding: 0;
    color: var(--mut);
    cursor: pointer;
    font: inherit;
  }

  .crumb:hover {
    color: var(--ink);
    text-decoration: underline;
  }

  .here {
    color: var(--ink-2);
  }

  .grow {
    flex: 1;
  }

  .moving {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: flex-end;
    margin: 10px 0;
    padding: 10px;
    border: 1px solid var(--rule);
    border-radius: 8px;
  }

  .moving label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font: 600 12px var(--ui);
    color: var(--mut);
  }

  h1 {
    font: 600 26px/1.2 var(--display);
    margin: 10px 0 2px;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .badge {
    font: 700 10px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #fff;
    background: var(--mop, var(--accent));
    border-radius: 5px;
    padding: 2px 6px;
  }

  .meta {
    font-size: 12.5px;
    color: var(--mut);
    margin: 0 0 12px;
  }

  .linkish {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    cursor: pointer;
    text-align: left;
  }

  .linkish:hover {
    text-decoration: underline;
  }

  .title-in {
    width: 100%;
    margin: 10px 0 2px;
    font: 600 24px/1.2 var(--display);
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 4px 8px;
    background: var(--paper);
    color: var(--ink);
  }

  .body-in {
    width: 100%;
    min-height: 420px;
    height: calc(100% - 120px);
    resize: vertical;
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 10px 12px;
    font: 14px/1.55 var(--mono);
    background: var(--paper);
    color: var(--ink);
  }

  .read {
    font-size: 14.5px;
    line-height: 1.55;
    user-select: text;
  }

  .read h2,
  .read h3,
  .read h4 {
    font-family: var(--display);
    margin: 14px 0 4px;
  }

  .read h2 {
    font-size: 19px;
  }

  .read h3 {
    font-size: 16.5px;
  }

  .read h4 {
    font-size: 15px;
  }

  .read p {
    margin: 2px 0;
  }

  .read .item {
    position: relative;
  }

  .read .mk {
    position: absolute;
    margin-left: -18px;
    color: var(--mut);
    min-width: 16px;
  }

  .read .gap {
    height: 8px;
  }

  pre,
  code {
    font-family: var(--mono);
    background: color-mix(in srgb, var(--ink) 6%, transparent);
    border-radius: 5px;
  }

  pre {
    padding: 8px 10px;
    margin: 6px 0;
    white-space: pre-wrap;
    font-size: 13px;
  }

  code {
    padding: 1px 5px;
    font-size: 13px;
  }

  .wlink {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    border-bottom: 1px dashed var(--accent);
    cursor: pointer;
  }

  .blank {
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    border: 1px dashed var(--warn);
    border-radius: 4px;
    padding: 0 4px;
    font: 600 13px var(--mono);
    color: var(--warn);
  }

  .empty {
    color: var(--mut);
    font-style: italic;
  }

  .welcome {
    max-width: 520px;
    margin: 40px auto;
    color: var(--ink-2);
  }

  .right {
    border-left: 1px solid var(--rule);
    padding: 12px;
    overflow: auto;
    font-size: 13px;
  }

  h5 {
    margin: 0 0 6px;
    font: 700 10.5px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .box {
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 8px 10px;
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .lockline {
    display: flex;
    gap: 6px;
    margin: 0;
  }

  .lockline .ic {
    padding-top: 2px;
  }

  .row-btn {
    display: flex;
    justify-content: space-between;
    gap: 6px;
  }

  .row-btn small {
    color: var(--mut);
  }

  .tk {
    display: flex;
    justify-content: space-between;
  }

  .tk .num {
    font: 700 12px var(--mono);
  }

  .st {
    font: 700 10px var(--ui);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--warn);
  }

  .st.done {
    color: var(--ok);
  }

  .versions button {
    border: 0;
    background: none;
    text-align: left;
    padding: 3px 6px;
    border-radius: 5px;
    cursor: pointer;
    font: 13px var(--ui);
    color: var(--ink);
  }

  .versions button.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .peek {
    max-height: 260px;
    overflow: auto;
    font-size: 12px;
  }

  @media (max-width: 1000px) {
    .wiki {
      grid-template-columns: 210px minmax(0, 1fr);
    }

    .right {
      display: none;
    }
  }

  @media (max-width: 700px) {
    .wiki {
      grid-template-columns: 1fr;
      height: auto;
    }

    .side {
      border-right: 0;
      border-bottom: 1px solid var(--rule);
      max-height: 40vh;
    }
  }
</style>
