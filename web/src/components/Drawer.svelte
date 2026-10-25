<script lang="ts">
  // The slide-out details drawer for one entry: title, hours stepper, project pills,
  // category, tickets (multi-paste), notes and every custom field that applies to entries.
  // Changes save as you go. Click outside, Esc or Done closes it.
  import { api } from '../lib/api';
  import type { CustomValue, Entry, EntryInput } from '../lib/api';
  import { longDay } from '../lib/format';
  import { copyText } from '../lib/protection.svelte';
  import { countTo } from '../lib/motion';
  import { fmtHours } from '../lib/format';
  import { app, attempt, flashEntry, projectColor, putEntry, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { trapTab } from '../lib/trap';
  import Icon from './Icon.svelte';
  import TicketChip from './TicketChip.svelte';

  const entry = $derived(app.entries.find((e) => e.id === app.drawer) ?? null);
  const open = $derived(entry !== null);
  const fields = $derived(app.fields.filter((f) => f.appliesTo === 'entry' && !f.hidden));

  let title = $state('');
  let notes = $state('');
  let ticketText = $state('');
  let categories = $state<string[]>([]);
  let values = $state<Record<string, string>>({});
  let links = $state<Record<string, string | null>>({});
  let loadedFor = '';
  let shown = $state(0);
  let shownFrom = 0;
  let titleEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    const e = entry;
    if (!e || loadedFor === e.id) return;
    loadedFor = e.id;
    title = e.description;
    notes = e.notes ?? '';
    ticketText = '';
    values = {};
    links = {};
    void attempt(() => api.listCustomValues(e.id)).then((list) => setValues(list ?? []));
    void attempt(() => api.categoryCodes()).then((list) => (categories = (list ?? []).map((c) => c.category)));
    queueMicrotask(() => titleEl?.focus());
  });

  $effect(() => {
    if (!open) loadedFor = '';
  });

  $effect(() => {
    const to = entry?.minutes ?? 0;
    return countTo(shownFrom, to, (v) => {
      shown = v;
      shownFrom = v;
    });
  });

  function setValues(list: CustomValue[]) {
    const v: Record<string, string> = {};
    const l: Record<string, string | null> = {};
    for (const item of list) {
      v[item.fieldId] = item.value;
      l[item.fieldId] = item.link;
    }
    values = v;
    links = l;
  }

  function inputOf(e: Entry, change: Partial<EntryInput> = {}): EntryInput {
    return {
      date: e.date,
      minutes: e.minutes,
      description: e.description,
      projectId: e.projectId,
      category: e.category,
      notes: e.notes,
      timesheetCodeId: e.timesheetCodeId,
      ...change,
    };
  }

  async function save(change: Partial<EntryInput>) {
    const e = entry;
    if (!e) return;
    const updated = await attempt(() => api.updateEntry(e.id, inputOf(e, change)));
    if (!updated) return;
    putEntry(updated);
    void refresh();
  }

  async function step(delta: number) {
    const e = entry;
    if (!e) return;
    const updated = await attempt(() => api.addEntryMinutes(e.id, delta));
    if (!updated) return;
    putEntry(updated);
    flashEntry(updated.id);
    void refresh();
  }

  async function addTickets(text: string) {
    const e = entry;
    if (!e || !text.trim()) return;
    const updated = await attempt(() => api.addEntryTickets(e.id, text));
    if (!updated) return;
    const added = updated.tickets.length - e.tickets.length;
    putEntry(updated);
    ticketText = '';
    toast(added > 0 ? `Added ${added} ticket${added === 1 ? '' : 's'}` : 'Those tickets are already on this entry.');
  }

  async function removeTicket(id: string, number: string) {
    const updated = await attempt(() => api.removeTicket(id));
    if (!updated) return;
    putEntry(updated);
    toast(`Removed ${number}`, 'Undo', () => {
      void attempt(() => api.addEntryTickets(updated.id, number)).then((u) => u && putEntry(u));
    });
  }

  async function setField(fieldId: string, value: string) {
    const e = entry;
    if (!e) return;
    const stored = await attempt(() => api.setCustomValue(fieldId, e.id, value));
    if (stored === undefined) return;
    values[fieldId] = stored;
    const list = await attempt(() => api.listCustomValues(e.id));
    if (list) setValues(list);
  }

  async function remove() {
    const e = entry;
    if (!e) return;
    const ok = await attempt(async () => {
      await api.deleteEntry(e.id);
      return true;
    });
    if (!ok) return;
    app.drawer = null;
    await refresh();
    toast(`Deleted “${e.description || 'the entry'}”`, 'Undo', () => {
      void attempt(() => api.restoreEntry(e.id)).then(refresh);
    }, () => void attempt(() => api.deleteEntry(e.id)).then(refresh));
  }

  function close() {
    app.drawer = null;
  }
  let drawerEl: HTMLElement | undefined = $state();
</script>

<div class="dscrim" class:on={open} onclick={close} aria-hidden="true"></div>
<svelte:window onkeydown={(e) => open && !e.defaultPrevented && trapTab(e, drawerEl)} />
<aside bind:this={drawerEl} class="paper drawer" class:on={open} aria-label="Entry details" aria-hidden={!open} style:--c={projectColor(entry?.projectId)}>
  {#if entry}
    {#key entry.id}
      <button class="x" type="button" aria-label="Close" onclick={close}><Icon name="x" size={18} /></button>
      <span class="lbl rise" style:--i={0}>{longDay(entry.date)}{entry.isTray ? ' · from the Tickets tray' : ''}</span>

      <div class="field rise" style:--i={1}>
        <label class="lbl" for="dr-title">What</label>
        <input
          id="dr-title"
          class="text-in"
          bind:this={titleEl}
          bind:value={title}
          onblur={() => title !== entry.description && save({ description: title })}
          onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
        />
      </div>

      <div class="field rise" style:--i={2}>
        <span class="lbl">Hours</span>
        <div class="step">
          <button class="hbtn" type="button" disabled={entry.isTray || entry.minutes === 0} onclick={() => step(-15)}>−15m</button>
          <span class="big mono" aria-live="polite">{fmtHours(shown)}</span>
          <button class="hbtn" type="button" disabled={entry.isTray} onclick={() => step(15)}>+15m</button>
          <button class="hbtn" type="button" disabled={entry.isTray} onclick={() => step(60)}>+1h</button>
        </div>
        {#if entry.isTray}<p class="note">These minutes add up the tray. Change them there.</p>{/if}
      </div>

      <div class="field rise" style:--i={3}>
        <span class="lbl">Project</span>
        <div class="pills">
          {#each app.projects.filter((p) => !p.archived || p.id === entry.projectId) as p (p.id)}
            <button type="button" style:--c={p.color ?? 'var(--none)'} aria-pressed={p.id === entry.projectId} onclick={() => save({ projectId: p.id })}>
              {p.name}
            </button>
          {/each}
          <button type="button" style:--c="var(--none)" aria-pressed={entry.projectId === null} onclick={() => save({ projectId: null })}>
            No project
          </button>
        </div>
      </div>

      <div class="field rise" style:--i={4}>
        <label class="lbl" for="dr-cat">Category</label>
        <select
          id="dr-cat"
          class="text-in"
          value={entry.category ?? ''}
          onchange={(e) => save({ category: (e.currentTarget as HTMLSelectElement).value || null })}
        >
          <option value="">None</option>
          {#each categories as c (c)}<option value={c}>{c}</option>{/each}
        </select>
      </div>

      <div class="field rise" style:--i={5}>
        <span class="lbl">Tickets · paste several at once</span>
        <div class="pills">
          {#each entry.tickets as t (t.id)}
            <TicketChip
              number={t.number}
              links={t.links}
              color={projectColor(entry.projectId)}
              onRemove={entry.isTray ? null : () => void removeTicket(t.id, t.number)}
            />
          {:else}
            <span class="note">No tickets yet.</span>
          {/each}
        </div>
        {#if !entry.isTray}
          <input
            class="text-in"
            bind:value={ticketText}
            placeholder="INC0045700, INC0045701 then Enter"
            aria-label="Add tickets"
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void addTickets(ticketText);
              }
            }}
            onpaste={(e) => {
              const pasted = e.clipboardData?.getData('text') ?? '';
              if (/[\n,;\t]/.test(pasted.trim())) {
                e.preventDefault();
                void addTickets(ticketText + ' ' + pasted);
              }
            }}
          />
        {/if}
      </div>

      <div class="field rise" style:--i={6}>
        <label class="lbl" for="dr-notes">Notes / next step</label>
        <textarea
          id="dr-notes"
          class="text-in"
          rows="3"
          bind:value={notes}
          onblur={() => notes !== (entry.notes ?? '') && save({ notes: notes || null })}
        ></textarea>
      </div>

      {#each fields as f, i (f.id)}
        <div class="field rise" style:--i={7 + i}>
          <label class="lbl" for="cf-{f.id}">{f.name}</label>
          {#if f.kind === 'choice'}
            <select
              id="cf-{f.id}"
              class="text-in"
              value={values[f.id] ?? ''}
              onchange={(e) => setField(f.id, (e.currentTarget as HTMLSelectElement).value)}
            >
              <option value="">—</option>
              {#each f.options as o (o)}<option value={o}>{o}</option>{/each}
            </select>
          {:else}
            <div class="with-copy">
              <input
                id="cf-{f.id}"
                class="text-in"
                inputmode={f.kind === 'number' ? 'decimal' : 'text'}
                value={values[f.id] ?? ''}
                placeholder={f.kind === 'link' ? (f.ticketPrefix ? `${f.ticketPrefix}123` : 'Key or text') : ''}
                onchange={(e) => setField(f.id, (e.currentTarget as HTMLInputElement).value)}
              />
              {#if f.kind === 'link' && links[f.id]}
                <button
                  type="button"
                  class="btn"
                  title={links[f.id] ?? ''}
                  onclick={() => copyText(links[f.id] ?? '').then((ok) => toast(ok ? 'Copied the link' : "Couldn't copy here."))}
                >
                  <Icon name="link" size={14} /> Copy link
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}

      <div class="foot rise" style:--i={8 + fields.length}>
        <button class="btn danger" type="button" onclick={remove}><Icon name="trash" size={14} /> Delete</button>
        <button class="btn primary" type="button" onclick={close}>Done</button>
      </div>
    {/key}
  {/if}
</aside>

<style>
  .dscrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: color-mix(in srgb, var(--desk) 35%, transparent);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.3s;
  }

  .dscrim.on {
    opacity: 1;
    pointer-events: auto;
  }

  .drawer {
    position: fixed;
    z-index: 31;
    top: 10px;
    bottom: 10px;
    right: 10px;
    width: min(410px, calc(100% - 20px));
    padding: 20px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    transform: translateX(calc(100% + 30px)) rotate(1.5deg);
    transition: transform calc(0.5s * var(--m)) var(--spring);
    box-shadow: var(--lift);
    border-top: 4px solid var(--c, var(--accent));
    visibility: hidden;
  }

  .drawer.on {
    transform: none;
    visibility: visible;
  }

  .drawer:not(.on) {
    transition:
      transform calc(0.4s * var(--m)) var(--ease),
      visibility 0s calc(0.4s * var(--m));
  }

  .drawer .rise {
    animation-delay: calc(var(--i, 0) * 40ms + 120ms);
  }

  .x {
    position: absolute;
    right: 12px;
    top: 12px;
    border: 0;
    background: none;
    cursor: pointer;
    color: var(--mut);
    padding: 4px;
    border-radius: 6px;
  }

  .x:hover {
    color: var(--ink);
  }

  #dr-title {
    font: 650 20px var(--display);
  }

  .step {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .step .big {
    font: 700 24px var(--mono);
    min-width: 70px;
    text-align: center;
  }

  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .pills {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .pills button {
    border: 1.5px solid color-mix(in srgb, var(--c) 45%, transparent);
    background: none;
    border-radius: 99px;
    padding: 3px 10px;
    cursor: pointer;
    font-size: 12.5px;
    color: var(--ink);
    transition:
      background 0.25s,
      color 0.25s;
  }

  .pills button[aria-pressed='true'] {
    background: var(--c);
    color: #fff;
    border-color: var(--c);
  }

  textarea {
    resize: vertical;
    font: 13.5px/1.6 var(--ui);
  }

  .with-copy {
    display: flex;
    gap: 6px;
  }

  .with-copy input {
    flex: 1;
  }

  .foot {
    display: flex;
    gap: 8px;
    margin-top: auto;
    justify-content: space-between;
  }
</style>
