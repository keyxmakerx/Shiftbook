<script lang="ts">
  // Customize: Look, Projects, Timesheet, Fields and Security in one place.
  import Icon from '../components/Icon.svelte';
  import type { IconName } from '../lib/icons';
  import Fields from './customize/Fields.svelte';
  import Look from './customize/Look.svelte';
  import Projects from './customize/Projects.svelte';
  import Security from './customize/Security.svelte';
  import TimesheetSettings from './customize/TimesheetSettings.svelte';

  type Section = 'look' | 'projects' | 'timesheet' | 'fields' | 'security';
  const SECTIONS: { id: Section; label: string; icon: IconName }[] = [
    { id: 'look', label: 'Look', icon: 'palette' },
    { id: 'projects', label: 'Projects', icon: 'folder' },
    { id: 'timesheet', label: 'Timesheet', icon: 'sheet' },
    { id: 'fields', label: 'Fields', icon: 'sliders' },
    { id: 'security', label: 'Security', icon: 'shield' },
  ];

  let { section = $bindable('look') }: { section?: Section } = $props();
</script>

<div class="cz">
  <nav class="paper side" aria-label="Customize sections">
    <h2>Customize</h2>
    {#each SECTIONS as s (s.id)}
      <button type="button" aria-current={section === s.id ? 'page' : undefined} onclick={() => (section = s.id)}>
        <Icon name={s.icon} size={16} />{s.label}
      </button>
    {/each}
  </nav>
  <div class="body">
    {#key section}
      <div class="sec rise">
        {#if section === 'look'}
          <Look />
        {:else if section === 'projects'}
          <Projects />
        {:else if section === 'timesheet'}
          <TimesheetSettings />
        {:else if section === 'fields'}
          <Fields />
        {:else}
          <Security />
        {/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .cz {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
    max-width: 1060px;
  }

  .side {
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    position: sticky;
    top: 0;
  }

  h2 {
    margin: 0 8px 8px;
    font: 650 22px var(--display);
  }

  .side button {
    display: flex;
    align-items: center;
    gap: 9px;
    border: 0;
    background: none;
    padding: 9px 10px;
    border-radius: 7px;
    cursor: pointer;
    text-align: left;
    color: var(--ink-2);
    font-size: 14px;
    font-weight: 500;
  }

  .side button:hover {
    background: color-mix(in srgb, var(--ink) 6%, transparent);
  }

  .side button[aria-current='page'] {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--ink);
    font-weight: 650;
  }

  .sec {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  @media (max-width: 760px) {
    .cz {
      grid-template-columns: 1fr;
    }
    .side {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
    }
    h2 {
      width: 100%;
    }
  }
</style>
