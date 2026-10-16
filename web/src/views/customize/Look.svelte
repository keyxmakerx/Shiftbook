<script lang="ts">
  // Customize → Look: theme, motion, accent and project colours. Changes show at once.
  import { api } from '../../lib/api';
  import type { PrefKey, Project } from '../../lib/api';
  import { TEXT_SIZES } from '../../lib/keys';
  import { ACCENTS, SWATCHES } from '../../lib/palette';
  import { app, applyPrefs, attempt, refreshProjects } from '../../lib/store.svelte';

  const prefs = $derived(app.prefs);
  const SIZE_NAMES: Record<number, string> = { 90: 'Small', 100: 'Normal', 115: 'Large', 130: 'Larger' };

  async function set(key: PrefKey, value: string) {
    const p = await attempt(() => api.savePref(key, value));
    if (!p) return;
    app.prefs = p;
    applyPrefs(p);
  }

  async function colour(p: Project, color: string) {
    const saved = await attempt(() =>
      api.updateProject(p.id, { name: p.name, color, defaultTimesheetCodeId: p.defaultTimesheetCodeId, archived: p.archived }),
    );
    if (saved) await attempt(refreshProjects);
  }

  let accentTimer: ReturnType<typeof setTimeout> | null = null;
  function liveAccent(value: string) {
    document.documentElement.style.setProperty('--accent', value);
    if (accentTimer !== null) clearTimeout(accentTimer);
    accentTimer = setTimeout(() => void set('ui.accent', value), 300);
  }
</script>

{#if prefs}
  <article class="paper cz-card">
    <h3>Theme</h3>
    <div class="seg" role="group" aria-label="Theme">
      <button type="button" aria-pressed={prefs.theme === 'system'} onclick={() => set('ui.theme', 'system')}>Follow system</button>
      <button type="button" aria-pressed={prefs.theme === 'light'} onclick={() => set('ui.theme', 'light')}>Light</button>
      <button type="button" aria-pressed={prefs.theme === 'dark'} onclick={() => set('ui.theme', 'dark')}>Dark</button>
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Motion</h3>
    <p class="sub">How much things slide and bounce. Your system's “reduce motion” setting always wins.</p>
    <div class="seg" role="group" aria-label="Motion">
      <button type="button" aria-pressed={prefs.motion === 'still'} onclick={() => set('ui.motion', 'still')}>Still</button>
      <button type="button" aria-pressed={prefs.motion === 'calm'} onclick={() => set('ui.motion', 'calm')}>Calm</button>
      <button type="button" aria-pressed={prefs.motion === 'lively'} onclick={() => set('ui.motion', 'lively')}>Lively</button>
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Text size</h3>
    <p class="sub">Change it any time with <span class="kbd">Ctrl +</span> and <span class="kbd">Ctrl −</span>; <span class="kbd">Ctrl 0</span> goes back to normal.</p>
    <div class="seg" role="group" aria-label="Text size">
      {#each TEXT_SIZES as size (size)}
        <button type="button" aria-pressed={prefs.textSize === size} onclick={() => set('ui.text_size', String(size))}>
          {SIZE_NAMES[size]}
        </button>
      {/each}
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Accent</h3>
    <p class="sub">Used for focus rings, the selected week and the tray.</p>
    <div class="cz-row">
      <button type="button" class="btn" aria-pressed={!prefs.accent} onclick={() => set('ui.accent', '')}>Built-in</button>
      <div class="accents" role="group" aria-label="Accent colours">
        {#each ACCENTS as c (c)}
          <button type="button" class="sw" style:--c={c} aria-label="Accent {c}" aria-pressed={prefs.accent === c} onclick={() => set('ui.accent', c)}></button>
        {/each}
      </div>
      <label class="check-row">
        <input type="color" class="color-in" value={prefs.accent ?? '#3d5bb0'} oninput={(e) => liveAccent((e.currentTarget as HTMLInputElement).value)} />
        Your own
      </label>
    </div>
  </article>

  <article class="paper cz-card">
    <h3>Project colours</h3>
    <p class="sub">Every bar, chip and line of a project uses its colour. Pick a swatch or your own.</p>
    {#each app.projects.filter((p) => !p.archived) as p (p.id)}
      <div class="cz-row">
        <i class="dot big-dot" style:--c={p.color ?? 'var(--none)'}></i>
        <span class="cz-grow">{p.name}</span>
        <div class="sws" role="group" aria-label="Colours for {p.name}">
          {#each SWATCHES as c (c)}
            <button type="button" class="sw" style:--c={c} aria-label="{p.name}: {c}" aria-pressed={p.color === c} onclick={() => colour(p, c)}></button>
          {/each}
        </div>
        <input
          type="color"
          class="color-in"
          aria-label="Your own colour for {p.name}"
          value={p.color ?? '#5a5f68'}
          onchange={(e) => colour(p, (e.currentTarget as HTMLInputElement).value)}
        />
      </div>
    {:else}
      <p class="empty-note">No projects yet. Add them under Projects.</p>
    {/each}
  </article>
{/if}

<style>
  .accents {
    display: flex;
    gap: 6px;
  }

  .big-dot {
    width: 14px;
    height: 14px;
  }
</style>
