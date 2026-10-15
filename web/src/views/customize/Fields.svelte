<script lang="ts">
  // Customize → Fields: your own fields on entries, tickets or projects. Link fields (for
  // example Jira) turn matching tickets into links you can copy.
  import type { FieldInput } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import FieldEditor from './FieldEditor.svelte';

  let preset = $state<FieldInput | null>(null);
  let presetKey = $state(0);

  function jira() {
    preset = {
      appliesTo: 'ticket',
      name: 'Jira',
      kind: 'link',
      options: '',
      linkTemplate: 'https://example.atlassian.net/browse/{ticket}',
      ticketPrefix: 'PROJ-',
      hidden: false,
    };
    presetKey += 1;
  }
</script>

<div class="intro">
  <p>Add the extra details you track. Entry fields show in the details drawer.</p>
  <button class="btn" type="button" onclick={jira}>Start a Jira link field</button>
</div>

{#each app.fields as f (f.id)}
  <FieldEditor field={f} />
{/each}

{#key presetKey}
  <FieldEditor field={null} {preset} />
{/key}

<style>
  .intro {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .intro p {
    margin: 0;
    color: var(--ink-2);
    flex: 1;
  }
</style>
