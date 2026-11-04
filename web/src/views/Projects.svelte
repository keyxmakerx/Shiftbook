<script lang="ts">
  // Projects (round 18, card r18-tabs): every project at a glance, from the Organise menu.
  // Its hours this week and in all, its tickets still open and the last day it was worked
  // (sums from crates/core/src/project.rs). Open its board, or look at its tickets. Renaming,
  // colours and timesheet codes stay in Settings → Projects.
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import type { ProjectGlance } from '../lib/api';
  import { openBoard } from '../lib/boards.svelte';
  import { relativeDay } from '../lib/format';
  import { phone } from '../lib/phone.svelte';
  import { app, attempt, refreshProjects, setView } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  let glances = $state<ProjectGlance[]>([]);
  let loaded = $state(false);
  let showArchived = $state(false);
  let name = $state('');

  const shown = $derived(glances.filter((g) => showArchived || !g.archived));
  const archivedCount = $derived(glances.filter((g) => g.archived).length);

  async function load() {
    glances = (await attempt(() => api.projectGlances(app.today))) ?? glances;
    loaded = true;
  }

  // Again whenever something changes elsewhere (time logged, a ticket ticked).
  $effect(() => {
    void app.changed;
    void app.today;
    void load();
  });

  async function add() {
    if (!name.trim()) return;
    const p = await attempt(() => api.createProject(name, null));
    if (!p) return;
    name = '';
    await attempt(refreshProjects);
    await load();
    toast(`Added ${p.name}`);
  }

  function tickets(id: string) {
    app.itemsTab = 'open';
    app.itemsProject = id;
    setView('items');
  }

  function settings() {
    app.settingsSection = 'projects';
    setView('settings');
  }
</script>

<section class="paper projects" aria-label="Projects">
  <header>
    <h1>Projects</h1>
    <span class="grow"></span>
    {#if archivedCount}
      <label class="arch"><input type="checkbox" bind:checked={showArchived} /> Show archived ({archivedCount})</label>
    {/if}
    <button type="button" class="btn" onclick={settings}><Icon name="sliders" size={14} /> Colours and codes</button>
  </header>

  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <input class="text-in" bind:value={name} placeholder="New project name" aria-label="New project name" />
    <button type="submit" class="btn primary" disabled={!name.trim()}><Icon name="plus" size={14} /> Add project</button>
  </form>

  {#if shown.length}
    <div class="grid">
      {#each shown as g (g.projectId)}
        <article class="card" class:archived={g.archived} style:--c={g.color ?? 'var(--accent)'}>
          <h2>{g.name}{#if g.archived}<span class="tag">Archived</span>{/if}</h2>
          <div class="nums">
            <span><b>{g.hoursWeek}</b> h this week</span>
            <span><b>{g.hoursAll}</b> h in all</span>
          </div>
          <p class="sub">
            {g.lastWorked ? `Last worked ${relativeDay(g.lastWorked, app.today).toLowerCase()}` : 'Not worked on yet'}
          </p>
          <div class="acts">
            {#if !phone.on}
              <button type="button" class="btn" onclick={() => openBoard(`project:${g.projectId}`)}>
                <Icon name="expand" size={14} /> Open its board
              </button>
            {/if}
            <button type="button" class="btn" class:warn={g.openTickets > 0} onclick={() => tickets(g.projectId)}>
              <Icon name="ticket" size={14} />
              {g.openTickets ? `${g.openTickets} ${g.openTickets === 1 ? 'ticket' : 'tickets'} open` : 'Tickets'}
            </button>
          </div>
        </article>
      {/each}
    </div>
  {:else if loaded}
    <p class="empty">No projects yet. Add one above, then file work under it from Log work or a card's menu.</p>
  {/if}
</section>

<style>
  .projects {
    max-width: 1180px;
    margin: 0 auto;
    padding: 16px 18px;
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin-bottom: 10px;
  }

  h1 {
    margin: 0;
    font: 700 22px var(--display);
  }

  .grow {
    flex: 1;
  }

  .arch {
    font-size: 13px;
    color: var(--ink-2);
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .add {
    display: flex;
    gap: 8px;
    margin-bottom: 14px;
    max-width: 480px;
  }

  .add .text-in {
    flex: 1;
    min-width: 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 12px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 8px;
    border-top: 4px solid var(--c);
    background: color-mix(in srgb, var(--c) 6%, var(--paper));
    box-shadow: var(--shadow);
  }

  .card.archived {
    opacity: 0.7;
  }

  h2 {
    margin: 0;
    font: 650 16px var(--display);
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .tag {
    font: 700 10px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .nums {
    display: flex;
    gap: 14px;
    font-size: 13px;
    color: var(--ink-2);
  }

  .nums b {
    font: 700 16px var(--mono);
    color: var(--ink);
  }

  .sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 2px;
  }

  .btn.warn {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 40%, transparent);
  }

  .empty {
    color: var(--mut);
  }
</style>
