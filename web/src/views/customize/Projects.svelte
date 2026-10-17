<script lang="ts">
  // Settings → Projects: add, rename, colour, default timesheet code, archive.
  // Right-click a project for its paper menu.
  import { api } from '../../lib/api';
  import type { Project, ProjectInput } from '../../lib/api';
  import { paperMenu, type MenuSpec } from '../../lib/menu.svelte';
  import { app, attempt, focusLog, refresh, refreshProjects } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';
  import Icon from '../../components/Icon.svelte';

  let name = $state('');
  const inputs: Record<string, HTMLInputElement> = {};

  function asInput(p: Project, change: Partial<ProjectInput> = {}): ProjectInput {
    return { name: p.name, color: p.color, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: p.archived, ...change };
  }

  async function add() {
    if (!name.trim()) return;
    const p = await attempt(() => api.createProject(name, null));
    if (!p) return;
    name = '';
    await attempt(refreshProjects);
    toast(`Added ${p.name}`);
  }

  async function save(p: Project, change: Partial<ProjectInput>) {
    const saved = await attempt(() => api.updateProject(p.id, asInput(p, change)));
    await attempt(refreshProjects);
    if (saved) void refresh();
    return saved;
  }

  async function archive(p: Project, archived: boolean) {
    const saved = await save(p, { archived });
    if (!saved) return;
    toast(archived ? `${p.name} is archived. Its history stays.` : `${p.name} is back`, 'Undo', () => {
      void save(saved, { archived: !archived });
    });
  }

  function menu(p: Project): MenuSpec {
    return {
      sections: [
        {
          head: p.name,
          items: [
            { label: 'Rename', icon: 'edit', side: true, act: () => inputs[p.id]?.select() },
            { label: 'Log time on it', icon: 'log', side: true, act: () => focusLog(`#${p.name.split(/\s+/)[0] ?? ''} `) },
            {
              label: p.archived ? 'Bring back' : 'Archive',
              icon: 'archive',
              top: true,
              act: () => void archive(p, !p.archived),
            },
          ],
        },
        { head: 'Colour', swatches: { current: p.color, pick: (c) => void save(p, { color: c }) } },
      ],
    };
  }
</script>

<article class="paper cz-card">
  <h3>Projects</h3>
  <p class="sub">Right-click a project for quick actions. Archived projects leave the pickers but keep their hours.</p>
  <form
    class="cz-row add"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <input class="text-in cz-grow" bind:value={name} placeholder="New project name" aria-label="New project name" />
    <button class="btn primary" type="submit" disabled={!name.trim()}><Icon name="plus" size={14} /> Add project</button>
  </form>

  {#each app.projects as p (p.id)}
    <div class="cz-row proj" class:archived={p.archived} use:paperMenu={() => menu(p)} style:--c={p.color ?? 'var(--none)'}>
      <input
        type="color"
        class="color-in"
        aria-label="Colour for {p.name}"
        value={p.color ?? '#5a5f68'}
        onchange={(e) => save(p, { color: (e.currentTarget as HTMLInputElement).value })}
      />
      <input
        class="text-in cz-grow name"
        value={p.name}
        aria-label="Name"
        bind:this={inputs[p.id]}
        onchange={(e) => save(p, { name: (e.currentTarget as HTMLInputElement).value })}
      />
      <label class="code">
        <span class="lbl">Default code</span>
        <select
          class="text-in"
          value={p.defaultTimesheetCodeId ?? ''}
          onchange={(e) => save(p, { defaultTimesheetCodeId: (e.currentTarget as HTMLSelectElement).value || null })}
        >
          <option value="">No code</option>
          {#each app.codes.filter((c) => !c.hidden || c.id === p.defaultTimesheetCodeId) as c (c.id)}
            <option value={c.id}>{c.area ? `${c.area} › ` : ''}{c.code}</option>
          {/each}
        </select>
      </label>
      <button class="btn" type="button" onclick={() => archive(p, !p.archived)}>
        <Icon name="archive" size={14} />{p.archived ? 'Bring back' : 'Archive'}
      </button>
    </div>
  {:else}
    <p class="empty-note">No projects yet. Add your first one above, for example “Tickets” or “Meetings”.</p>
  {/each}
</article>

<style>
  .proj {
    border-left: 3px solid var(--c);
    padding-left: 10px;
    border-radius: 4px;
    background-color: var(--paper);
  }

  .proj.archived {
    opacity: 0.6;
  }

  .name {
    font-weight: 600;
  }

  .code {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .code select {
    max-width: 220px;
  }
</style>
