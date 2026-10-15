<script lang="ts">
  // One custom field's settings (or a new field when `field` is null).
  import { api } from '../../lib/api';
  import type { CustomField, FieldInput } from '../../lib/api';
  import { app, attempt } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';
  import Icon from '../../components/Icon.svelte';

  let { field = null, preset = null }: { field?: CustomField | null; preset?: FieldInput | null } = $props();

  const blank: FieldInput = {
    appliesTo: 'entry',
    name: '',
    kind: 'text',
    options: '',
    linkTemplate: '',
    ticketPrefix: '',
    hidden: false,
  };

  function fromField(f: CustomField | null): FieldInput {
    if (!f) return { ...(preset ?? blank) };
    return {
      appliesTo: f.appliesTo,
      name: f.name,
      kind: f.kind,
      options: f.options.join('\n'),
      linkTemplate: f.linkTemplate ?? '',
      ticketPrefix: f.ticketPrefix ?? '',
      hidden: f.hidden,
    };
  }

  // A local copy to edit; reset when the field (or preset) changes.
  let draft = $state<FieldInput>({ ...blank });
  $effect(() => {
    draft = fromField(field);
  });

  const isNew = $derived(field === null);
  const dirty = $derived(JSON.stringify(draft) !== JSON.stringify(fromField(field)));
  const sample = $derived(
    draft.linkTemplate.includes('{ticket}')
      ? draft.linkTemplate.replace('{ticket}', `${draft.ticketPrefix || 'PROJ-'}123`)
      : '',
  );

  async function reload() {
    const list = await attempt(() => api.listCustomFields());
    if (list) app.fields = list;
  }

  async function save() {
    const saved = await attempt(() => api.saveCustomField(field?.id ?? null, draft));
    if (!saved) return;
    await reload();
    if (isNew) draft = { ...blank };
    toast(isNew ? `Added the field “${saved.name}”` : 'Saved');
  }

  async function remove() {
    if (!field) return;
    const f = field;
    const ok = await attempt(async () => {
      await api.deleteCustomField(f.id);
      return true;
    });
    if (!ok) return;
    await reload();
    toast(`Removed the field “${f.name}”`);
  }
</script>

<form
  class="paper cz-card editor"
  class:new={isNew}
  onsubmit={(e) => {
    e.preventDefault();
    void save();
  }}
>
  {#if isNew}<h3>Add a field</h3>{/if}
  <div class="cols">
    <label class="field">
      <span class="lbl">Name</span>
      <input class="text-in" bind:value={draft.name} placeholder="e.g. Jira, Site, Size" />
    </label>
    <label class="field">
      <span class="lbl">Type</span>
      <select class="text-in" bind:value={draft.kind}>
        <option value="text">Text</option>
        <option value="number">Number</option>
        <option value="choice">Choice from a list</option>
        <option value="link">Link (e.g. Jira)</option>
      </select>
    </label>
    <label class="field">
      <span class="lbl">Shows on</span>
      <select class="text-in" bind:value={draft.appliesTo}>
        <option value="entry">Entries</option>
        <option value="ticket">Tickets</option>
        <option value="project">Projects</option>
      </select>
    </label>
  </div>

  {#if draft.kind === 'choice'}
    <label class="field">
      <span class="lbl">Choices · one per line</span>
      <textarea class="text-in" rows="3" bind:value={draft.options} placeholder={'Small\nMedium\nLarge'}></textarea>
    </label>
  {/if}

  {#if draft.kind === 'link'}
    <div class="cols two">
      <label class="field">
        <span class="lbl">Link template · {'{ticket}'} is replaced</span>
        <input class="text-in mono" bind:value={draft.linkTemplate} placeholder={'https://example.atlassian.net/browse/{ticket}'} spellcheck="false" />
      </label>
      <label class="field">
        <span class="lbl">Ticket prefix</span>
        <input class="text-in mono" bind:value={draft.ticketPrefix} placeholder="PROJ-" spellcheck="false" />
      </label>
    </div>
    <p class="note">
      Tickets that start with the prefix get a “Copy link” option. Links are only shown and copied;
      Shiftbook never opens them or goes online.
      {#if sample}<br /><span class="lbl">Example</span> <span class="mono">{sample}</span>{/if}
    </p>
  {/if}

  <div class="foot">
    {#if !isNew}
      <label class="check-row"><input type="checkbox" bind:checked={draft.hidden} /> Hidden</label>
      <button class="btn danger" type="button" onclick={remove}><Icon name="trash" size={14} /> Remove</button>
    {/if}
    <button class="btn primary" type="submit" disabled={!draft.name.trim() || (!isNew && !dirty)}>
      <Icon name={isNew ? 'plus' : 'check'} size={14} />{isNew ? 'Add field' : 'Save'}
    </button>
  </div>
</form>

<style>
  .editor.new {
    border-top: 3px solid var(--accent);
  }

  .cols {
    display: grid;
    grid-template-columns: 1.4fr 1fr 1fr;
    gap: 10px;
  }

  .cols.two {
    grid-template-columns: 2fr 1fr;
  }

  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .foot {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    align-items: center;
  }

  textarea {
    resize: vertical;
  }

  @media (max-width: 760px) {
    .cols,
    .cols.two {
      grid-template-columns: 1fr;
    }
  }
</style>
