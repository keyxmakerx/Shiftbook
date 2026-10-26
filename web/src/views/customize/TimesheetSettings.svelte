<script lang="ts">
  // Settings → Timesheet: your timesheet codes (area, phase, code, order, hide), which
  // code each category gets, the comment format, the default grouping and the week start day.
  import { api } from '../../lib/api';
  import type { CategoryCode, CodeInput, PrefKey, TimesheetCode, WeekStartDay } from '../../lib/api';
  import { app, attempt, refreshRibbon } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';
  import Icon from '../../components/Icon.svelte';
  import CodeSelect from '../../components/CodeSelect.svelte';

  let categories = $state<CategoryCode[]>([]);
  let draft = $state<CodeInput>({ area: '', phaseLabel: '', code: '', sortOrder: 0, hidden: false });
  let separator = $state(app.prefs?.comment.separator ?? '; ');

  $effect(() => {
    void attempt(() => api.categoryCodes()).then((c) => (categories = c ?? []));
  });

  async function reloadCodes() {
    const codes = await attempt(() => api.listTimesheetCodes());
    if (codes) app.codes = codes;
  }

  function inputOf(c: TimesheetCode, change: Partial<CodeInput>): CodeInput {
    return { area: c.area, phaseLabel: c.phaseLabel, code: c.code, sortOrder: c.sortOrder, hidden: c.hidden, ...change };
  }

  async function update(c: TimesheetCode, change: Partial<CodeInput>) {
    await attempt(() => api.saveTimesheetCode(c.id, inputOf(c, change)));
    await reloadCodes();
  }

  async function add() {
    if (!draft.code.trim()) return;
    const saved = await attempt(() => api.saveTimesheetCode(null, draft));
    if (!saved) return;
    draft = { area: draft.area, phaseLabel: draft.phaseLabel, code: '', sortOrder: 0, hidden: false };
    await reloadCodes();
    toast(`Added ${saved.code}`);
  }

  async function mapCategory(category: string, codeId: string | null) {
    await attempt(() => api.setCategoryCode(category, codeId));
    categories = (await attempt(() => api.categoryCodes())) ?? categories;
  }

  async function pref(key: PrefKey, value: string) {
    const p = await attempt(() => api.savePref(key, value));
    if (p) app.prefs = p;
  }

  const WEEK_DAYS: { value: WeekStartDay; label: string }[] = [
    { value: 'monday', label: 'Monday' },
    { value: 'tuesday', label: 'Tuesday' },
    { value: 'wednesday', label: 'Wednesday' },
    { value: 'thursday', label: 'Thursday' },
    { value: 'friday', label: 'Friday' },
    { value: 'saturday', label: 'Saturday' },
    { value: 'sunday', label: 'Sunday' },
  ];

  async function setWeekStart(day: string) {
    await pref('calendar.week_start', day);
    await attempt(refreshRibbon);
  }

  const example = $derived(
    app.prefs
      ? ['Firewall cleanup', 'Ticket queue']
          .map((t, i) => (app.prefs?.comment.includeTickets ? `${t} (${i ? 'INC0045678, INC0045690' : 'CHG0012345'})` : t))
          .join(app.prefs.comment.separator)
      : '',
  );
</script>

<article class="paper cz-card">
  <h3>Timesheet codes</h3>
  <p class="sub">The codes you type into your timesheet, in the order you enter them. Hidden codes stay on old entries.</p>
  <div class="grid head" aria-hidden="true">
    <span class="lbl">Area</span><span class="lbl">Phase</span><span class="lbl">Code</span><span class="lbl">Order</span><span class="lbl">Hide</span>
  </div>
  {#each app.codes as c (c.id)}
    <div class="grid" class:hidden-code={c.hidden}>
      <input class="text-in" value={c.area} aria-label="Area" onchange={(e) => update(c, { area: (e.currentTarget as HTMLInputElement).value })} />
      <input class="text-in" value={c.phaseLabel} aria-label="Phase" onchange={(e) => update(c, { phaseLabel: (e.currentTarget as HTMLInputElement).value })} />
      <input class="text-in strong" value={c.code} aria-label="Code" onchange={(e) => update(c, { code: (e.currentTarget as HTMLInputElement).value })} />
      <input
        class="text-in"
        type="number"
        value={c.sortOrder}
        aria-label="Order"
        onchange={(e) => update(c, { sortOrder: Number((e.currentTarget as HTMLInputElement).value) || 0 })}
      />
      <input type="checkbox" class="hide" checked={c.hidden} aria-label="Hide {c.code}" onchange={(e) => update(c, { hidden: (e.currentTarget as HTMLInputElement).checked })} />
    </div>
  {:else}
    <p class="empty-note">No codes yet. Add the ones from your timesheet system, for example Area “Operations”, Code “1 - Support”.</p>
  {/each}
  <form
    class="grid add"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <input class="text-in" bind:value={draft.area} placeholder="Area" aria-label="New code area" />
    <input class="text-in" bind:value={draft.phaseLabel} placeholder="Phase" aria-label="New code phase" />
    <input class="text-in" bind:value={draft.code} placeholder="Code" aria-label="New code" />
    <button class="btn primary" type="submit" disabled={!draft.code.trim()}><Icon name="plus" size={14} /> Add</button>
  </form>
</article>

<article class="paper cz-card">
  <h3>Default codes</h3>
  <p class="sub">An entry without its own code, and whose project has none, gets one of these.</p>
  {#if app.prefs}
    <div class="cz-row">
      <span class="cz-grow"><strong>Tickets</strong> <span class="from">from the Tickets tray</span></span>
      <CodeSelect
        value={app.prefs.ticketsCode}
        label="Code for tickets"
        empty="Unassigned"
        onchange={(id) => pref('timesheet.tickets_code', id ?? '')}
      />
    </div>
  {/if}
  {#each categories as cat (cat.category)}
    <div class="cz-row">
      <span class="cz-grow">{cat.category}</span>
      <CodeSelect value={cat.codeId} label="Code for {cat.category}" empty="Unassigned" onchange={(id) => mapCategory(cat.category, id)} />
    </div>
  {/each}
  <p class="sub">Tickets with a project use the project's code. A code chosen on an entry always wins.</p>
</article>

{#if app.prefs}
  <article class="paper cz-card">
    <h3>Comments and grouping</h3>
    <label class="check-row">
      <input
        type="checkbox"
        checked={app.prefs.comment.includeTickets}
        onchange={(e) => pref('timesheet.comment_tickets', String((e.currentTarget as HTMLInputElement).checked))}
      />
      Put ticket numbers in brackets after each title
    </label>
    <div class="cz-row">
      <span class="cz-grow">Between items</span>
      <div class="seg" role="group" aria-label="Separator">
        {#each ['; ', ' / ', ' | ', ', '] as s (s)}
          <button
            type="button"
            aria-pressed={app.prefs.comment.separator === s}
            onclick={() => {
              separator = s;
              void pref('timesheet.comment_separator', s);
            }}><span class="mono">{s.trim()}</span></button
          >
        {/each}
      </div>
      <input
        class="text-in sep-in mono"
        bind:value={separator}
        maxlength="5"
        aria-label="Your own separator"
        onchange={() => separator && pref('timesheet.comment_separator', separator)}
      />
    </div>
    <p class="example"><span class="lbl">Example</span> {example}</p>
    <div class="cz-row">
      <span class="cz-grow">Group the crib sheet</span>
      <div class="seg" role="group" aria-label="Default grouping">
        <button type="button" aria-pressed={app.prefs.groupBy === 'code'} onclick={() => pref('timesheet.group_by', 'code')}>By timesheet code</button>
        <button type="button" aria-pressed={app.prefs.groupBy === 'project'} onclick={() => pref('timesheet.group_by', 'project')}>By project</button>
      </div>
    </div>
    <div class="cz-row">
      <span class="cz-grow">My week starts on</span>
      <select
        class="text-in"
        value={app.prefs.weekStart}
        aria-label="My week starts on"
        onchange={(e) => setWeekStart((e.currentTarget as HTMLSelectElement).value)}
      >
        {#each WEEK_DAYS as d (d.value)}
          <option value={d.value}>{d.label}</option>
        {/each}
      </select>
    </div>
    <p class="sub">Changes the weeks in the ribbon, the crib sheet and words like “next week”.</p>
  </article>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1.3fr) 70px 40px;
    gap: 8px;
    align-items: center;
  }

  .grid.add {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1.3fr) 118px;
    padding-top: 8px;
    border-top: 1px solid var(--rule);
  }

  .grid.head {
    margin-bottom: -6px;
  }

  .hidden-code input:not(.hide) {
    opacity: 0.5;
  }

  .strong {
    font-weight: 600;
  }

  .hide {
    width: 18px;
    height: 18px;
    justify-self: center;
    accent-color: var(--accent);
  }

  .from {
    color: var(--mut);
    font-size: 13px;
  }

  .sep-in {
    width: 70px;
  }

  .example {
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--paper-2);
  }
</style>
