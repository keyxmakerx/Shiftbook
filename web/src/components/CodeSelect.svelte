<script lang="ts">
  // A timesheet code picker. Codes can be long ("Operations › Phase 2 - Infrastructure
  // maintenance and patching"), so the select never grows past its box and the chosen code
  // is written out in full underneath, where it wraps instead of being cut off.
  import { app } from '../lib/store.svelte';
  import type { TimesheetCode } from '../lib/api';

  let {
    value,
    label,
    empty = 'No code',
    onchange,
  }: {
    /** The chosen code id, or null for the empty choice. */
    value: string | null;
    /** What the select is for, read out by screen readers. */
    label: string;
    /** The words for the empty choice, e.g. "Unassigned" or "Use the usual code (…)". */
    empty?: string;
    onchange: (codeId: string | null) => void;
  } = $props();

  function full(c: TimesheetCode): string {
    return c.area ? `${c.area} › ${c.code}` : c.code;
  }

  // Hidden codes are only offered when already chosen.
  const choices = $derived(app.codes.filter((c) => !c.hidden || c.id === value));
  const chosen = $derived(app.codes.find((c) => c.id === value) ?? null);
  const shown = $derived(chosen ? full(chosen) : empty);
  // Short enough to read in the box: no need to write it out again.
  const LONG = 32;
</script>

<div class="code-pick">
  <select
    class="text-in"
    value={value ?? ''}
    aria-label={label}
    title={shown}
    onchange={(e) => onchange((e.currentTarget as HTMLSelectElement).value || null)}
  >
    <option value="">{empty}</option>
    {#each choices as c (c.id)}
      <option value={c.id}>{full(c)}</option>
    {/each}
  </select>
  {#if shown.length > LONG}
    <span class="full" aria-hidden="true">{shown}</span>
  {/if}
</div>

<style>
  .code-pick {
    flex: 1 1 240px;
    min-width: 0;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  select {
    width: 100%;
    max-width: 100%;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow: hidden;
  }

  .full {
    font-size: 12px;
    color: var(--ink-2);
    white-space: normal;
    overflow-wrap: anywhere;
    padding: 0 2px;
  }
</style>
