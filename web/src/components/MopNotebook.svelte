<script lang="ts">
  // The MOP notebook (round 16, card r15-mopbook): a Notepad++-style window with a tab per
  // MOP. Numbered shows line numbers (a toggle), light colours and the counts; Minimal is
  // just the text; Checklist puts a box by each line, and ticking one crosses it out with
  // the section under it. The text stays editable in every mode and saves as you type.
  // Templates fill their [[blanks]] into a new MOP. The rules are in crates/core/src/mop.rs.
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { CheckLine, MopView, Note } from '../lib/api';
  import { counts, findAll, hasFiles, paint } from '../lib/mop';
  import {
    book,
    closeMopBook,
    closeTab,
    moveBook,
    openMopBook,
    pickMopFiles,
    offerFiles,
    resizeBook,
    setBookMode,
    setBookView,
    showTab,
    tabInfo,
    toggleNumbers,
  } from '../lib/mopbook.svelte';
  import { openMenu } from '../lib/menu.svelte';
  import type { MenuItem } from '../lib/menu.svelte';
  import { app, attempt, openNote, words } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { phone } from '../lib/phone.svelte';
  import { dragButton } from '../lib/rightDrag';
  import { focusNow } from '../lib/focus';
  import Icon from './Icon.svelte';

  const SAVE_AFTER_MS = 700;
  const LINE = 20;
  const PAD = { top: 10, left: 12 };
  const VIEWS: { view: MopView; label: string; tip: string }[] = [
    { view: 'numbered', label: 'Numbered', tip: 'Line numbers, colours and counts, like Notepad++' },
    { view: 'minimal', label: 'Minimal', tip: 'Just the text' },
    { view: 'checklist', label: 'Checklist', tip: 'Tick lines off; a ticked heading or step crosses out the section under it' },
  ];

  interface Doc {
    note: Note;
    text: string;
    saved: string;
  }

  let docs = $state<Record<string, Doc>>({});
  /** The newer text written elsewhere while a tab had unsaved typing, by tab. */
  let clashes = $state<Record<string, string>>({});
  let gone = $state<Record<string, boolean>>({});
  let status = $state<'' | 'saving' | 'saved'>('');
  let timer: ReturnType<typeof setTimeout> | null = null;
  let ta: HTMLTextAreaElement | undefined = $state();
  let sx = $state(0);
  let sy = $state(0);
  let sel = $state({ start: 0, end: 0 });
  let checks = $state<CheckLine[]>([]);
  let live = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let renaming = $state<string | null>(null);
  let renameText = $state('');
  let finding = $state(false);
  let query = $state('');
  let hit = $state(0);
  let filling = $state<{ id: string; blanks: string[]; values: Record<string, string>; title: string } | null>(null);
  let warnSave = $state(false);
  let dropping = $state(false);

  const doc = $derived(book.active ? docs[book.active] : undefined);
  const view = $derived(book.view);
  const lines = $derived(doc ? doc.text.split('\n') : []);
  const painted = $derived(doc && view !== 'minimal' ? paint(doc.text) : []);
  const stats = $derived(doc ? counts(doc.text, sel.start, sel.end) : null);
  const gutterCh = $derived(String(lines.length).length);
  const checked = $derived(checks.length === lines.length ? checks : []);
  const tickedCount = $derived(checked.filter((c) => c.ticked).length);
  const box = $derived(live ?? { x: book.x, y: book.y, w: book.w, h: book.h });

  // Line start offsets, for placing Find's marks.
  const starts = $derived.by(() => {
    const out = [0];
    const text = doc?.text ?? '';
    for (let i = text.indexOf('\n'); i >= 0; i = text.indexOf('\n', i + 1)) out.push(i + 1);
    return out;
  });
  const matches = $derived(finding && doc ? findAll(doc.text, query) : []);
  const marks = $derived(
    matches.map((at) => {
      let lo = 0;
      let hi = starts.length - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (starts[mid]! <= at) lo = mid;
        else hi = mid - 1;
      }
      return { line: lo, col: visualCol(lines[lo] ?? '', at - starts[lo]!), len: query.length };
    }),
  );

  /** The column a character sits at on screen, with tabs every 4. */
  function visualCol(line: string, upTo: number): number {
    let col = 0;
    for (const c of line.slice(0, upTo)) col = c === '\t' ? col + 4 - (col % 4) : col + 1;
    return col;
  }

  // Load each open tab's MOP once.
  $effect(() => {
    for (const id of book.tabs) {
      if (docs[id] || gone[id]) continue;
      void api.getNote(id).then(
        (n) => {
          docs[id] = { note: n, text: n.body, saved: n.body };
          tabInfo.titles[id] = n.title;
        },
        () => (gone[id] = true),
      );
    }
  });

  // Checklist mode asks the app which lines are ticked and crossed out.
  $effect(() => {
    if (view !== 'checklist' || !doc) return;
    const id = doc.note.id;
    const text = doc.text;
    const ticks = [...doc.note.ticks];
    const t = setTimeout(async () => {
      const got = await attempt(() => api.mopChecklist(text, ticks));
      if (got && book.active === id) checks = got;
    }, 120);
    return () => clearTimeout(t);
  });

  onMount(() => () => {
    if (timer) clearTimeout(timer);
    void saveAll(true);
  });

  /**
   * Save a tab's typing. If the MOP was written somewhere else since it loaded here (the Notes
   * view, a note window, a sync), nothing is overwritten: the tab asks which to keep.
   * `closing` keeps both instead (the newer text, then this tab's), as nothing is left to ask in.
   */
  async function save(id: string, closing = false) {
    const d = docs[id];
    if (!d || (d.text === d.saved && clashes[id] === undefined)) return;
    status = 'saving';
    const now = await attempt(() => api.getNote(id));
    if (!now) {
      status = '';
      return;
    }
    let body = d.text;
    if (now.body !== d.saved && now.body !== d.text) {
      if (!closing) {
        clashes[id] = now.body;
        status = '';
        return;
      }
      body = `${now.body}\n\n${d.text}`;
    }
    delete clashes[id];
    // The MOP's current title, project and tags, in case they changed elsewhere.
    const updated = await attempt(() =>
      api.updateNote(id, { kind: now.kind, title: now.title, body, projectId: now.projectId, tags: now.tags.join(', ') }),
    );
    if (updated) {
      d.saved = body;
      if (closing) d.text = body;
      d.note = updated;
      tabInfo.titles[id] = updated.title;
      tabInfo.dirty[id] = d.text !== body;
      status = 'saved';
    } else {
      status = '';
    }
  }

  /** Overwrite the newer text with this tab's. */
  function keepMine(id: string) {
    const d = docs[id];
    const newer = clashes[id];
    if (!d || newer === undefined) return;
    d.saved = newer;
    delete clashes[id];
    void save(id);
  }

  /** Drop this tab's typing and show the newer text. */
  function useNewer(id: string) {
    const d = docs[id];
    const newer = clashes[id];
    if (!d || newer === undefined) return;
    d.text = d.saved = newer;
    delete clashes[id];
    tabInfo.dirty[id] = false;
  }

  // Something changed elsewhere: reload the tabs with nothing typed waiting.
  $effect(() => {
    void app.changed;
    untrack(() => {
      if (timer) return;
      for (const [id, d] of Object.entries(docs)) {
        if (d.text !== d.saved || clashes[id] !== undefined) continue;
        void api.getNote(id).then(
          (n) => {
            const now = docs[id];
            if (!now || now.text !== now.saved) return;
            now.note = n;
            now.text = now.saved = n.body;
            tabInfo.titles[id] = n.title;
          },
          () => undefined,
        );
      }
    });
  });

  async function saveAll(closing = false) {
    timer = null;
    for (const id of Object.keys(docs)) await save(id, closing);
  }

  /** Save now if anything is waiting (before ticking, template work or switching tabs). */
  async function flush() {
    if (timer) clearTimeout(timer);
    await saveAll();
  }

  function typed(e: Event) {
    if (!doc) return;
    doc.text = (e.currentTarget as HTMLTextAreaElement).value;
    tabInfo.dirty[doc.note.id] = doc.text !== doc.saved;
    track();
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void saveAll(), SAVE_AFTER_MS);
  }

  function track() {
    if (ta) sel = { start: ta.selectionStart, end: ta.selectionEnd };
  }

  function scrolled() {
    if (!ta) return;
    sx = ta.scrollLeft;
    sy = ta.scrollTop;
  }

  async function switchTo(id: string) {
    await flush();
    showTab(id);
    sel = { start: 0, end: 0 };
    checks = [];
    sx = sy = 0;
  }

  async function close(id: string) {
    if (timer) clearTimeout(timer);
    timer = null;
    await save(id, true);
    await saveAll();
    closeTab(id);
    delete docs[id];
    if (!book.tabs.length) closeMopBook();
  }

  async function tick(i: number) {
    const d = doc;
    const key = checked[i]?.key;
    if (!d || !key) return;
    await flush();
    const ticks = d.note.ticks.includes(key) ? d.note.ticks.filter((k) => k !== key) : [...d.note.ticks, key];
    d.note = { ...d.note, ticks };
    const n = await attempt(() => api.setNoteTicks(d.note.id, ticks));
    if (n) d.note = { ...n, body: d.note.body };
  }

  async function clearTicks() {
    const d = doc;
    if (!d) return;
    const before = d.note.ticks;
    d.note = { ...d.note, ticks: [] };
    const n = await attempt(() => api.setNoteTicks(d.note.id, []));
    if (!n) return;
    toast('Ticks cleared for a fresh run', 'Undo', () => {
      void attempt(() => api.setNoteTicks(d.note.id, before)).then((back) => back && (d.note = back));
    });
  }

  /** Ctrl+Enter in Checklist mode ticks the line the cursor is on. */
  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      e.stopPropagation();
      openFind();
    } else if (view === 'checklist' && (e.ctrlKey || e.metaKey) && e.key === 'Enter' && doc) {
      e.preventDefault();
      void tick(doc.text.slice(0, sel.start).split('\n').length - 1);
    }
  }

  function openFind() {
    finding = true;
    if (doc && sel.end > sel.start) {
      const picked = doc.text.slice(sel.start, sel.end);
      if (!picked.includes('\n')) query = picked;
    }
    hit = 0;
  }

  function jump(step: number) {
    if (!matches.length || !ta) return;
    hit = (hit + step + matches.length) % matches.length;
    const m = marks[hit]!;
    ta.scrollTop = Math.max(0, m.line * LINE - ta.clientHeight / 2);
  }

  function endFind() {
    finding = false;
    const at = matches[hit];
    if (ta && at !== undefined) {
      ta.focus();
      ta.setSelectionRange(at, at + query.length);
      track();
    }
  }

  async function setTemplate(on: boolean) {
    const d = doc;
    if (!d) return;
    if (!(await attempt(() => api.setNoteTemplate(d.note.id, on).then(() => true)))) return;
    d.note = { ...d.note, template: on };
    toast(on ? 'Now a template. Use "New MOP from this" to fill in its [[blanks]].' : 'Back to a plain MOP.');
  }

  async function startFill(id: string) {
    await flush();
    const d = docs[id] ?? (await attempt(() => api.getNote(id)).then((n) => n && { note: n, text: n.body, saved: n.body }));
    if (!d) return;
    const blanks = await attempt(() => api.mopBlanks(d.text));
    if (!blanks) return;
    filling = { id, blanks, values: Object.fromEntries(blanks.map((b) => [b, ''])), title: '' };
  }

  async function fill() {
    const f = filling;
    if (!f) return;
    const values = f.blanks.map((name) => ({ name, value: f.values[name] ?? '' }));
    const made = await attempt(() => api.noteFromTemplate(f.id, f.title, values));
    if (!made) return;
    filling = null;
    openMopBook(made.id);
    const left = values.filter((v) => !v.value.trim()).length;
    toast(left ? `New MOP made. ${left} blank${left === 1 ? '' : 's'} left to fill in, shown highlighted.` : 'New MOP made from the template.');
  }

  async function saveFile() {
    warnSave = false;
    const d = doc;
    if (!d) return;
    await flush();
    const name = await attempt(() => api.saveMopFile(d.note.id));
    if (name) toast(`Saved as ${name}`);
  }

  async function newMop() {
    const made = await attempt(() =>
      api.createNote({ kind: 'mop', title: `New ${words().mop}`, body: '', projectId: null, tags: '' }),
    );
    if (made) {
      openMopBook(made.id);
      queueMicrotask(() => ta?.focus());
    }
  }

  async function plusMenu(e: MouseEvent) {
    const at = e.currentTarget as HTMLElement;
    const list = await attempt(() => api.listNotes('', 'mop', false));
    const mops = list?.notes ?? [];
    const templates = mops.filter((n) => n.template);
    const others = mops.filter((n) => !n.template && !book.tabs.includes(n.id)).slice(0, 12);
    openMenu(at, {
      sections: [
        {
          items: [
            { label: `New ${words().mop}`, icon: 'plus', act: () => void newMop() },
            { label: `Add ${words().mop} files…`, icon: 'upload', act: pickMopFiles },
          ],
        },
        ...(templates.length
          ? [{ head: 'New from a template', items: templates.map((t): MenuItem => ({ label: t.title, icon: 'copy', act: () => void startFill(t.id) })) }]
          : []),
        ...(others.length ? [{ head: 'Open', items: others.map((n): MenuItem => ({ label: n.title, icon: 'note', act: () => openMopBook(n.id) })) }] : []),
      ],
    });
  }

  function tabMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    const d = docs[id];
    if (!d) return;
    openMenu(e.currentTarget as HTMLElement, {
      sections: [
        {
          items: [
            { label: 'Rename', icon: 'edit', act: () => startRename(id) },
            d.note.template
              ? { label: `New ${words().mop} from this template`, icon: 'copy', act: () => void startFill(id) }
              : { label: 'Use as template', icon: 'copy', act: () => void switchTo(id).then(() => setTemplate(true)) },
            ...(d.note.template ? [{ label: 'Stop being a template', icon: 'x' as const, act: () => void switchTo(id).then(() => setTemplate(false)) }] : []),
            { label: 'Save as a text file…', icon: 'save', act: () => void switchTo(id).then(() => (warnSave = true)) },
            { label: 'Open in Notes', icon: 'open', act: () => void flush().then(() => openNote(id)) },
            { label: 'Close tab', icon: 'x', act: () => void close(id) },
          ],
        },
      ],
    });
  }

  function startRename(id: string) {
    renaming = id;
    renameText = docs[id]?.note.title ?? '';
  }

  async function rename() {
    const id = renaming;
    renaming = null;
    const d = id ? docs[id] : undefined;
    if (!id || !d || !renameText.trim() || renameText.trim() === d.note.title) return;
    const n = d.note;
    const updated = await attempt(() =>
      api.updateNote(id, { kind: n.kind, title: renameText, body: d.text, projectId: n.projectId, tags: n.tags.join(', ') }),
    );
    if (updated) {
      d.note = updated;
      d.saved = updated.body;
      tabInfo.titles[id] = updated.title;
    }
  }

  function drag(e: PointerEvent, resize: boolean) {
    if (!dragButton(e) || book.mode === 'full') return;
    if (!resize && (e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    const start = { x: e.clientX, y: e.clientY };
    const from = { x: book.x, y: book.y, w: book.w, h: book.h };
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      const dx = m.clientX - start.x;
      const dy = m.clientY - start.y;
      live = resize
        ? { ...from, w: Math.max(360, from.w + dx), h: Math.max(220, from.h + dy) }
        : { ...from, x: from.x + dx, y: Math.max(0, from.y + dy) };
    };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
      if (live) {
        if (resize) resizeBook(live.w, live.h);
        else moveBook(live.x, live.y);
      }
      live = null;
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }

  function tone(t: string): string {
    return t ? `t-${t}` : '';
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="win paper"
  class:rolled={book.mode === 'rolled'}
  class:full={book.mode === 'full'}
  class:moving={!!live}
  class:dropping
  hidden={book.mode === 'min'}
  style:left="{box.x}px"
  style:top="{box.y}px"
  style:width="{box.w}px"
  style:height={book.mode === 'rolled' ? 'auto' : `${box.h}px`}
  aria-label={words().book}
  onkeydown={onKey}
  ondragover={(e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    e.stopPropagation();
    dropping = true;
  }}
  ondragleave={(e) => {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) dropping = false;
  }}
  ondrop={(e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    e.stopPropagation();
    dropping = false;
    if (e.dataTransfer?.files.length) void offerFiles(e.dataTransfer.files);
  }}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="bar" onpointerdown={(e) => drag(e, false)} ondblclick={() => !phone.on && setBookMode(book.mode === 'full' ? 'open' : 'full')}>
    <Icon name="book" size={15} />
    <b class="ttl">{words().book}</b>
    {#if doc}<span class="sub">{doc.note.title}{doc.note.template ? ' · template' : ''}</span>{/if}
    <span class="gap"></span>
    {#if !phone.on}
      <button type="button" title={book.mode === 'rolled' ? 'Open it again' : 'Roll up to the title bar'} aria-label={book.mode === 'rolled' ? 'Unroll' : 'Roll up'} onclick={() => setBookMode(book.mode === 'rolled' ? 'open' : 'rolled')}>
        <Icon name={book.mode === 'rolled' ? 'down' : 'up'} size={14} />
      </button>
      <button type="button" title="Minimise to the bottom bar" aria-label="Minimise" onclick={() => setBookMode('min')}>
        <Icon name="minus" size={14} />
      </button>
      <button type="button" title={book.mode === 'full' ? 'Back to a window' : 'Full screen'} aria-label={book.mode === 'full' ? 'Restore' : 'Full screen'} onclick={() => setBookMode(book.mode === 'full' ? 'open' : 'full')}>
        <Icon name={book.mode === 'full' ? 'restore' : 'expand'} size={14} />
      </button>
    {/if}
    <button type="button" class="x" title="Close the notebook (your MOPs stay in Notes)" aria-label="Close" onclick={() => void flush().then(closeMopBook)}>
      <Icon name="x" size={14} />
    </button>
  </header>

  {#if book.mode !== 'rolled'}
    <div class="tabs" role="tablist" aria-label="Open {words().mop}s">
      {#each book.tabs as id (id)}
        {#if renaming === id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            aria-label="New name"
            bind:value={renameText}
            use:focusNow
            onkeydown={(e) => {
              if (e.key === 'Enter') void rename();
              if (e.key === 'Escape') renaming = null;
            }}
            onblur={() => void rename()}
          />
        {:else}
          <div class="tab" class:on={book.active === id} class:dirty={tabInfo.dirty[id]}>
            <button
              type="button"
              role="tab"
              class="tname"
              aria-selected={book.active === id}
              title="{tabInfo.titles[id] ?? ''}: double-click to rename, right-click for more"
              onclick={() => void switchTo(id)}
              ondblclick={() => startRename(id)}
              oncontextmenu={(e) => tabMenu(e, id)}
            >
              {#if docs[id]?.note.template}<span class="tpl" title="Template">T</span>{/if}
              <span class="tl">{tabInfo.titles[id] ?? (gone[id] ? 'Gone' : '…')}</span>
              <span class="dot" aria-hidden="true"></span>
            </button>
            <button type="button" class="tx" aria-label="Close {tabInfo.titles[id] ?? 'tab'}" title="Close tab" onclick={() => void close(id)}>
              <Icon name="x" size={11} />
            </button>
          </div>
        {/if}
      {/each}
      <button type="button" class="plus" title="New {words().mop}, from a template, or add files" aria-label="New tab" onclick={plusMenu}>
        <Icon name="plus" size={14} />
      </button>
    </div>

    <div class="tools" role="toolbar" aria-label="Notebook tools">
      <div class="seg" role="group" aria-label="How it shows">
        {#each VIEWS as v (v.view)}
          <button type="button" aria-pressed={view === v.view} title={v.tip} onclick={() => setBookView(v.view)}>{v.label}</button>
        {/each}
      </div>
      {#if view !== 'minimal'}
        <button type="button" class="tool" aria-pressed={book.numbers} title="Show or hide line numbers" onclick={toggleNumbers}>
          <span class="mono">#</span> Line numbers
        </button>
      {/if}
      {#if view === 'checklist' && doc}
        <span class="ticked">{tickedCount} ticked</span>
        <button type="button" class="tool" disabled={!doc.note.ticks.length} title="Untick everything for a fresh run" onclick={clearTicks}>Clear ticks</button>
      {/if}
      <span class="gap"></span>
      {#if doc}
        <button type="button" class="tool" title="Find (Ctrl+F)" aria-label="Find" onclick={openFind}><Icon name="search" size={14} /></button>
        {#if doc.note.template}
          <button type="button" class="tool strong" title="Make a new {words().mop} and fill in the [[blanks]]" onclick={() => doc && void startFill(doc.note.id)}>
            <Icon name="copy" size={14} /> New from this
          </button>
        {:else}
          <button type="button" class="tool" title="Keep this as a template with [[blanks]] to fill in" onclick={() => void setTemplate(true)}>
            <Icon name="copy" size={14} /> Use as template
          </button>
        {/if}
        <button type="button" class="tool" title="Save a copy as a .txt file" aria-label="Save as a text file" onclick={() => (warnSave = true)}><Icon name="save" size={14} /></button>
      {/if}
    </div>

    {#if finding}
      <div class="find">
        <Icon name="search" size={14} />
        <input
          class="text-in"
          placeholder="Find…"
          aria-label="Find in this {words().mop}"
          bind:value={query}
          use:focusNow
          oninput={() => (hit = 0)}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              jump(e.shiftKey ? -1 : 1);
            } else if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              endFind();
            }
          }}
        />
        <span class="mut">{query ? (matches.length ? `${hit + 1} of ${matches.length}` : 'No match') : ''}</span>
        <button type="button" class="tool" aria-label="Previous" onclick={() => jump(-1)}><Icon name="up" size={13} /></button>
        <button type="button" class="tool" aria-label="Next" onclick={() => jump(1)}><Icon name="down" size={13} /></button>
        <button type="button" class="tool" aria-label="Close Find" onclick={endFind}><Icon name="x" size={13} /></button>
      </div>
    {/if}

    {#if !book.tabs.length}
      <div class="empty">
        <p>No {words().mop}s open. Start one, or drop your {words().mop} files here to copy them in.</p>
        <div class="row">
          <button type="button" class="btn primary" onclick={() => void newMop()}><Icon name="plus" size={15} /> New {words().mop}</button>
          <button type="button" class="btn" onclick={pickMopFiles}><Icon name="upload" size={15} /> Add {words().mop} files…</button>
        </div>
      </div>
    {:else if book.active && gone[book.active]}
      <p class="empty">This {words().mop} is no longer here. It may have been deleted.</p>
    {:else if !doc}
      <p class="empty">Opening…</p>
    {:else if view === 'minimal'}
      {#if clashes[doc.note.id] !== undefined}
        {@const id = doc.note.id}
        <p class="clash strip" role="alert">
          Changed somewhere else since you opened this.
          <button type="button" class="tool" onclick={() => keepMine(id)}>Keep mine</button>
          <button type="button" class="tool" onclick={() => useNewer(id)}>Show the newer one</button>
        </p>
      {/if}
      <textarea
        bind:this={ta}
        class="plain"
        value={doc.text}
        placeholder="Write the steps…"
        aria-label={doc.note.title}
        spellcheck="false"
        oninput={typed}
        onselect={track}
        onkeyup={track}
        onpointerup={track}
      ></textarea>
    {:else}
      <div class="ed" style:--gw="{book.numbers ? gutterCh + 2 : 0}ch" class:checks={view === 'checklist'}>
        {#if book.numbers || view === 'checklist'}
          <div class="gut" aria-hidden={view !== 'checklist'}>
            <div class="gin" style:transform="translateY({-sy}px)">
              {#each lines as _, i (i)}
                <div class="gl" class:cur={stats?.line === i + 1}>
                  {#if view === 'checklist'}
                    {#if checked[i]?.key}
                      <button
                        type="button"
                        class="cb"
                        class:on={checked[i]?.ticked}
                        class:under={checked[i]?.crossed && !checked[i]?.ticked}
                        aria-pressed={checked[i]?.ticked}
                        aria-label="Tick line {i + 1}"
                        onclick={() => void tick(i)}
                      >{#if checked[i]?.ticked}<Icon name="check" size={11} />{/if}</button>
                    {:else}
                      <span class="cb none"></span>
                    {/if}
                  {/if}
                  {#if book.numbers}<span class="num">{i + 1}</span>{/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}
        <div class="area">
          <div class="paint" aria-hidden="true" style:transform="translate({-sx}px, {-sy}px)">
            {#each marks as m, i (i)}
              <span
                class="mark"
                class:now={i === hit}
                style:top="{PAD.top + m.line * LINE}px"
                style:left="calc({PAD.left}px + {m.col}ch)"
                style:width="{m.len}ch"
              ></span>
            {/each}
            {#each painted as pieces, i (i)}
              <div class="pl" class:x={checked[i]?.crossed}>{#each pieces as p, j (j)}<span class={tone(p.tone)}>{p.text}</span>{/each}</div>
            {/each}
          </div>
          <textarea
            bind:this={ta}
            class="src"
            value={doc.text}
            wrap="off"
            placeholder="# Prep&#10;1. First step&#10;Verify: what you expect"
            aria-label={doc.note.title}
            spellcheck="false"
            oninput={typed}
            onscroll={scrolled}
            onselect={track}
            onkeyup={track}
            onpointerup={track}
          ></textarea>
        </div>
      </div>
      <footer class="status">
        {#if stats}
          <span>Ln {stats.line}, Col {stats.col}</span>
          <span>{stats.lines.toLocaleString()} lines</span>
          <span>{stats.chars.toLocaleString()} chars</span>
          <span class:hl={stats.selChars > 0}>Sel {stats.selChars.toLocaleString()}{stats.selLines > 1 ? ` (${stats.selLines} lines)` : ''}</span>
        {/if}
        <span class="gap"></span>
        {#if clashes[doc.note.id] !== undefined}
          {@const id = doc.note.id}
          <span class="clash" role="alert">
            Changed somewhere else since you opened this.
            <button type="button" onclick={() => keepMine(id)}>Keep mine</button>
            <button type="button" onclick={() => useNewer(id)}>Show the newer one</button>
          </span>
        {:else}
          <span aria-live="polite">{status === 'saving' ? 'Saving…' : status === 'saved' ? 'Saved, encrypted' : 'Saves as you type'}</span>
        {/if}
        {#if book.mode !== 'full'}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span class="grip" title="Drag to resize" onpointerdown={(e) => drag(e, true)}></span>
        {/if}
      </footer>
    {/if}
    {#if view === 'minimal' && book.mode !== 'full'}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="grip corner" title="Drag to resize" onpointerdown={(e) => drag(e, true)}></span>
    {/if}

    {#if filling}
      <div class="over" role="dialog" aria-label="New {words().mop} from a template">
        <div class="sheet paper">
          <h3>New {words().mop} from “{docs[filling.id]?.note.title ?? 'template'}”</h3>
          {#if filling.blanks.length}
            <p class="mut">Fill in each blank once; it changes everywhere it appears. Leave one empty to fill it in later (it stays highlighted).</p>
            <div class="fields">
              {#each filling.blanks as b, i (b)}
                <label class="field">
                  <span class="lbl blank">[[{b}]]</span>
                  {#if i === 0}
                    <input class="text-in" bind:value={filling.values[b]} use:focusNow onkeydown={(e) => e.key === 'Enter' && void fill()} />
                  {:else}
                    <input class="text-in" bind:value={filling.values[b]} onkeydown={(e) => e.key === 'Enter' && void fill()} />
                  {/if}
                </label>
              {/each}
            </div>
          {:else}
            <p class="mut">This template has no [[blanks]] yet, so you'll get a plain copy. Type [[a name]] wherever something changes each time.</p>
          {/if}
          <label class="field">
            <span class="lbl">Title (blank uses the template's, filled in)</span>
            <input class="text-in" bind:value={filling.title} onkeydown={(e) => e.key === 'Enter' && void fill()} />
          </label>
          <div class="row end">
            <button type="button" class="btn" onclick={() => (filling = null)}>Cancel</button>
            <button type="button" class="btn primary" onclick={() => void fill()}>Make the {words().mop}</button>
          </div>
        </div>
      </div>
    {/if}

    {#if warnSave}
      <div class="over" role="alertdialog" aria-label="Save as a text file">
        <div class="sheet paper">
          <h3>Save a copy as a text file?</h3>
          <p>The file you save is <b>not encrypted</b>. Anyone who can open the folder you pick can read it. Your {words().mop} in Shiftbook stays encrypted.</p>
          <div class="row end">
            <button type="button" class="btn" onclick={() => (warnSave = false)}>Cancel</button>
            <button type="button" class="btn primary" use:focusNow onclick={() => void saveFile()}>Save the file</button>
          </div>
        </div>
      </div>
    {/if}

    {#if dropping}
      <div class="dropzone" aria-hidden="true">
        <Icon name="upload" size={22} />
        <b>Drop to copy these in</b>
        <span>Each file becomes an encrypted {words().mop}. Your files stay where they are.</span>
      </div>
    {/if}
  {/if}
</section>

<style>
  .win {
    position: absolute;
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    overflow: hidden;
    box-shadow:
      0 0 0 1px var(--rule),
      0 30px 60px -20px rgba(0, 0, 0, 0.6);
    pointer-events: auto;
    --mop: #8a4c7b;
    animation: pop calc(0.22s * var(--m)) var(--spring);
    transition:
      left calc(0.25s * var(--m)) var(--ease),
      top calc(0.25s * var(--m)) var(--ease),
      width calc(0.25s * var(--m)) var(--ease),
      height calc(0.25s * var(--m)) var(--ease);
  }

  .win.moving {
    transition: none;
  }

  .win[hidden] {
    display: none;
  }

  .win.full {
    left: 0 !important;
    top: 0 !important;
    width: 100% !important;
    height: 100% !important;
    border-radius: 0;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.97);
    }
  }

  .bar {
    flex: none;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 6px 0 12px;
    background: color-mix(in srgb, var(--mop) 13%, var(--paper));
    border-bottom: 1px solid var(--rule);
    cursor: grab;
    user-select: none;
    touch-action: none;
    color: var(--mop);
  }

  .full .bar {
    cursor: default;
  }

  .ttl {
    font: 650 13.5px var(--display);
    color: var(--ink);
    white-space: nowrap;
  }

  .sub {
    font-size: 11.5px;
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .gap {
    flex: 1;
  }

  .bar button {
    width: 28px;
    height: 24px;
    border: 0;
    background: none;
    border-radius: 6px;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
    flex: none;
  }

  .bar button:hover {
    background: color-mix(in srgb, var(--ink) 9%, transparent);
    color: var(--ink);
  }

  .bar .x:hover {
    background: var(--bad);
    color: #fff;
  }

  .tabs {
    flex: none;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 5px 8px 0;
    background: var(--paper-2);
    border-bottom: 1px solid var(--rule);
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .tab {
    display: flex;
    align-items: center;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: 7px 7px 0 0;
    max-width: 200px;
    flex: none;
  }

  .tab.on {
    background: var(--paper);
    border-color: var(--rule);
    margin-bottom: -1px;
  }

  .tname {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 5px 4px 5px 10px;
    border: 0;
    background: none;
    font: 12.5px var(--ui);
    color: var(--mut);
    cursor: pointer;
  }

  .tab.on .tname {
    color: var(--ink);
    font-weight: 600;
  }

  .tname .tl {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tpl {
    font: 700 9.5px var(--ui);
    color: #fff;
    background: var(--mop);
    border-radius: 3px;
    padding: 1px 3px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
    display: none;
    flex: none;
  }

  .tab.dirty .dot {
    display: inline-block;
  }

  .tx,
  .plus {
    border: 0;
    background: none;
    color: var(--mut);
    border-radius: 5px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .tx {
    width: 20px;
    height: 20px;
    margin-right: 4px;
    opacity: 0.6;
  }

  .plus {
    width: 28px;
    height: 26px;
    margin-bottom: 2px;
  }

  .tx:hover,
  .plus:hover {
    background: color-mix(in srgb, var(--ink) 9%, transparent);
    color: var(--ink);
    opacity: 1;
  }

  .rename {
    font: 12.5px var(--ui);
    padding: 4px 8px;
    width: 160px;
    border: 1px solid var(--accent);
    border-radius: 6px;
    margin-bottom: 2px;
  }

  .tools,
  .find {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-bottom: 1px solid var(--rule);
    background: var(--paper);
    flex-wrap: wrap;
  }

  .seg {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    overflow: hidden;
  }

  .seg button {
    border: 0;
    background: none;
    padding: 4px 10px;
    font: 600 12px var(--ui);
    color: var(--ink-2);
    cursor: pointer;
  }

  .seg button + button {
    border-left: 1px solid var(--line-strong);
  }

  .seg button[aria-pressed='true'] {
    background: color-mix(in srgb, var(--mop) 16%, var(--paper));
    color: var(--ink);
  }

  .tool {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid transparent;
    background: none;
    border-radius: 7px;
    padding: 4px 8px;
    font: 12px var(--ui);
    color: var(--ink-2);
    cursor: pointer;
  }

  .tool:hover:not(:disabled) {
    background: color-mix(in srgb, var(--ink) 7%, transparent);
    color: var(--ink);
  }

  .tool[aria-pressed='true'] {
    border-color: var(--line-strong);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--ink);
  }

  .tool.strong {
    color: var(--mop);
    font-weight: 650;
  }

  .tool:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .ticked {
    font-size: 12px;
    color: var(--mut);
  }

  .find .text-in {
    width: 200px;
    padding: 4px 8px;
  }

  .mut {
    color: var(--mut);
    font-size: 12px;
  }

  .empty {
    flex: 1;
    display: grid;
    place-content: center;
    gap: 12px;
    text-align: center;
    color: var(--mut);
    padding: 20px;
    margin: 0;
  }

  .row {
    display: flex;
    gap: 8px;
    justify-content: center;
    flex-wrap: wrap;
  }

  .row.end {
    justify-content: flex-end;
  }

  .plain {
    flex: 1;
    min-height: 0;
    resize: none;
    border: 0;
    padding: 18px 22px;
    font: 15px/1.65 var(--ui);
    color: var(--ink);
    background: var(--paper);
  }

  .full .plain {
    padding: 26px max(24px, calc(50% - 380px));
  }

  .plain:focus,
  .src:focus {
    outline: none;
  }

  .ed {
    flex: 1;
    min-height: 0;
    display: flex;
    background: var(--paper);
    font: 13px/20px var(--mono);
    tab-size: 4;
  }

  .gut {
    flex: none;
    overflow: hidden;
    background: var(--paper-2);
    border-right: 1px solid var(--rule);
    color: var(--mut);
    user-select: none;
  }

  .gin {
    padding: 10px 0;
  }

  .gl {
    height: 20px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px;
    justify-content: flex-end;
  }

  .gl.cur .num {
    color: var(--ink);
    font-weight: 700;
  }

  .num {
    min-width: var(--gw);
    text-align: right;
    font-size: 12px;
  }

  .cb {
    width: 15px;
    height: 15px;
    border: 1.5px solid var(--line-strong);
    border-radius: 4px;
    background: var(--paper);
    display: grid;
    place-items: center;
    padding: 0;
    cursor: pointer;
    color: #fff;
    margin-right: auto;
  }

  .cb.on {
    background: var(--ok);
    border-color: var(--ok);
  }

  .cb.under {
    border-style: dashed;
    opacity: 0.55;
  }

  .cb.none {
    border-color: transparent;
    background: none;
    cursor: default;
  }

  .area {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .paint {
    position: absolute;
    top: 0;
    left: 0;
    padding: 10px 12px;
    color: var(--ink);
    pointer-events: none;
    min-width: 100%;
  }

  .pl {
    height: 20px;
    white-space: pre;
  }

  .pl.x {
    color: var(--mut);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, var(--ok) 70%, var(--mut));
    background: color-mix(in srgb, var(--ok) 6%, transparent);
  }

  .t-h {
    color: var(--mop);
    font-weight: 700;
  }

  .t-rb {
    color: var(--bad);
    font-weight: 700;
  }

  .t-num {
    color: var(--accent);
    font-weight: 700;
  }

  .t-v {
    color: var(--ok);
  }

  .t-blank {
    background: var(--warn-wash);
    color: var(--warn);
    border-radius: 3px;
  }

  .x .t-h,
  .x .t-rb,
  .x .t-num,
  .x .t-v {
    color: inherit;
  }

  .mark {
    position: absolute;
    height: 20px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-radius: 2px;
  }

  .mark.now {
    background: color-mix(in srgb, var(--warn) 45%, transparent);
    outline: 1px solid var(--warn);
  }

  .src {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    resize: none;
    border: 0;
    margin: 0;
    padding: 10px 12px;
    font: inherit;
    tab-size: 4;
    white-space: pre;
    overflow: auto;
    color: transparent;
    caret-color: var(--ink);
    background: transparent;
  }

  .src::placeholder {
    color: var(--mut);
  }

  .src::selection {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .status {
    flex: none;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 4px 10px;
    font: 11.5px var(--ui);
    color: var(--mut);
    border-top: 1px solid var(--rule);
    background: var(--paper-2);
    white-space: nowrap;
    overflow: hidden;
  }

  .clash {
    color: var(--warn);
    font-weight: 600;
  }

  .clash button {
    margin-left: 6px;
  }

  .clash.strip {
    margin: 0;
    padding: 5px 10px;
    font-size: 12px;
    border-bottom: 1px solid var(--rule);
    background: var(--warn-wash);
  }

  .status .hl {
    color: var(--ink);
    font-weight: 600;
  }

  .grip {
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    border-right: 2px solid var(--line-strong);
    border-bottom: 2px solid var(--line-strong);
    border-radius: 0 0 4px 0;
    touch-action: none;
    flex: none;
  }

  .grip.corner {
    position: absolute;
    right: 4px;
    bottom: 4px;
  }

  .over {
    position: absolute;
    inset: 36px 0 0;
    background: color-mix(in srgb, var(--ink) 25%, transparent);
    display: grid;
    place-items: center;
    padding: 16px;
    z-index: 2;
  }

  .sheet {
    width: min(440px, 100%);
    max-height: 100%;
    overflow: auto;
    border-radius: 12px;
    padding: 16px 18px;
    box-shadow: var(--shadow);
    display: grid;
    gap: 10px;
  }

  .sheet h3 {
    margin: 0;
    font: 650 16px var(--display);
  }

  .sheet p {
    margin: 0;
  }

  .fields {
    display: grid;
    gap: 8px;
  }

  .field {
    display: grid;
    gap: 3px;
  }

  .lbl {
    font-size: 12px;
    color: var(--mut);
  }

  .lbl.blank {
    font: 600 12px var(--mono);
    text-transform: none;
    letter-spacing: 0;
    color: var(--warn);
  }

  .dropzone {
    position: absolute;
    inset: 36px 0 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    text-align: center;
    background: color-mix(in srgb, var(--mop) 12%, var(--paper));
    border: 2px dashed var(--mop);
    border-radius: 0 0 12px 12px;
    color: var(--mop);
    z-index: 3;
    pointer-events: none;
  }

  .dropzone span {
    color: var(--ink-2);
    font-size: 12.5px;
    max-width: 300px;
  }

  @media (prefers-reduced-motion: reduce) {
    .win {
      animation: none;
      transition: none;
    }
  }
</style>
