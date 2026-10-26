<script lang="ts">
  // Items (round 15, card r15-things): one place for every ticket and chore, open or done.
  // Tickets come from their items (one per ticket number, however many days and entries it's
  // on); chores are the repeating things. Open, Follow up and Done split them; the project and
  // kind chips and the find box narrow them. It takes over the Done tickets board: Done lists
  // every ticket ticked done. Tasks join when Tasks are built.
  import { onMount } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import type { CardKind, ItemSummary, Recurring } from '../lib/api';
  import { bringUpSection, followWords } from '../lib/bringUp';
  import { fmtMinutes, relativeDay } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { openRepeating, refreshRepeating, repeating } from '../lib/repeating.svelte';
  import { app, attempt, projectById, projectColor, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { openWindow } from '../lib/windows.svelte';

  type Kind = 'ticket' | 'chore';
  type Row = {
    key: string;
    kind: Kind;
    name: string;
    title: string;
    projectId: string | null;
    home: string;
    minutes: number;
    last: string | null;
    /** The follow-up day (tickets) or when it's due (chores). */
    follow: string | null;
    followText: string;
    /** Needs following up now: the day has come, or the chore is due soon or late. */
    now: boolean;
    done: boolean;
    ticket: ItemSummary | null;
    chore: Recurring | null;
  };

  let tickets = $state<ItemSummary[]>([]);
  let find = $state('');
  let project = $state<'all' | 'none' | string>('all');
  let kind = $state<'all' | Kind>('all');

  async function load() {
    tickets = (await attempt(() => api.items())) ?? tickets;
  }

  onMount(() => {
    void load();
    void refreshRepeating();
  });

  function ticketFollow(t: ItemSummary): string | null {
    const days = t.cards.map((c) => app.followUps[c]).filter((d): d is string => !!d);
    return days.length ? days.sort()[0]! : null;
  }

  const rows = $derived.by((): Row[] => {
    const out: Row[] = tickets.map((t) => {
      const follow = ticketFollow(t);
      const p = projectById(t.projectId);
      const thing = t.recurringId ? repeating.items.find((r) => r.id === t.recurringId) : undefined;
      return {
        key: `item:${t.id}`,
        kind: 'ticket',
        name: t.number,
        title: t.title,
        projectId: t.projectId,
        home: p?.name ?? (thing ? `⟳ ${thing.name}` : ''),
        minutes: t.minutes,
        last: t.last,
        follow,
        followText: follow ? (follow <= app.today ? (follow === app.today ? 'Today' : `Since ${relativeDay(follow, app.today)}`) : relativeDay(follow, app.today)) : '',
        now: !!follow && follow <= app.today,
        done: t.done,
        ticket: t,
        chore: null,
      };
    });
    for (const r of repeating.items) {
      if (r.state === 'inactive') continue;
      const due = r.state === 'overdue' || r.state === 'due_soon' || r.state === 'renewed';
      out.push({
        key: `chore:${r.id}`,
        kind: 'chore',
        name: r.name,
        title: r.kind ?? '',
        projectId: null,
        home: '',
        minutes: r.totalMinutes,
        last: r.lastDone,
        follow: r.nextDue,
        followText: r.state === 'renewed' ? 'New date needed' : r.dueWords ? `Due ${r.dueWords}` : '',
        now: due,
        done: false,
        ticket: null,
        chore: r,
      });
    }
    return out;
  });

  const filtered = $derived.by(() => {
    const q = find.trim().toLowerCase();
    return rows.filter(
      (r) =>
        (kind === 'all' || r.kind === kind) &&
        (project === 'all' || (project === 'none' ? !r.projectId : r.projectId === project)) &&
        (!q || `${r.name} ${r.title} ${r.home}`.toLowerCase().includes(q)),
    );
  });

  const open = $derived(filtered.filter((r) => !r.done));
  const followList = $derived(open.filter((r) => r.follow && (r.kind === 'ticket' || r.now)));
  const doneList = $derived(filtered.filter((r) => r.done));

  const byFollowThenLast = (a: Row, b: Row) =>
    Number(b.now) - Number(a.now) ||
    (a.now && b.now ? (a.follow ?? '').localeCompare(b.follow ?? '') : 0) ||
    (b.last ?? '').localeCompare(a.last ?? '');

  const shown = $derived(
    app.itemsTab === 'follow'
      ? [...followList].sort((a, b) => (a.follow ?? '').localeCompare(b.follow ?? ''))
      : app.itemsTab === 'done'
        ? [...doneList].sort((a, b) => (b.last ?? '').localeCompare(a.last ?? ''))
        : [...open].sort(byFollowThenLast),
  );

  const projects = $derived(app.projects.filter((p) => !p.archived && rows.some((r) => r.projectId === p.id)));

  function cardOf(r: Row): [CardKind, string] | null {
    const c = r.ticket?.cards[0];
    return c ? (c.split(':') as [CardKind, string]) : null;
  }

  function openRow(r: Row) {
    if (r.chore) return openRepeating(r.chore.id);
    const card = cardOf(r);
    if (card) openWindow(card[0], card[1]);
  }

  function followMenu(anchor: HTMLElement, r: Row) {
    const card = cardOf(r);
    if (!card) return;
    openMenu(anchor, { sections: [bringUpSection(card[0], card[1], r.name)] });
  }

  async function tick(r: Row) {
    const t = r.ticket;
    if (!t) return;
    const done = !t.done;
    const ok = await attempt(() => api.itemSetDone(t.id, done, app.today).then(() => true));
    if (!ok) return;
    await Promise.all([load(), refresh()]);
    toast(done ? `${t.number} done` : `${t.number} open again`, 'Undo', () => {
      void attempt(() => api.itemSetDone(t.id, !done, app.today)).then(() => Promise.all([load(), refresh()]));
    });
  }

  const TABS: { id: typeof app.itemsTab; label: string; count: () => number }[] = [
    { id: 'open', label: 'Open', count: () => open.length },
    { id: 'follow', label: 'Follow up', count: () => followList.length },
    { id: 'done', label: 'Done', count: () => doneList.length },
  ];
</script>

<section class="paper items" aria-label="Items">
  <header>
    <h1>Items</h1>
    <div class="seg tabs" role="tablist" aria-label="Which items">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" aria-selected={app.itemsTab === t.id} onclick={() => (app.itemsTab = t.id)}>
          {t.label}
          {#if t.id !== 'done'}<span class="n">{t.count()}</span>{/if}
        </button>
      {/each}
    </div>
    <span class="grow"></span>
    <input class="text-in find" type="search" bind:value={find} placeholder="Find a ticket or chore…" aria-label="Find an item" />
  </header>

  <div class="chips" role="group" aria-label="Filter">
    <button type="button" class="chip" aria-pressed={project === 'all'} onclick={() => (project = 'all')}>All projects</button>
    {#each projects as p (p.id)}
      <button type="button" class="chip" style:--c={projectColor(p.id)} aria-pressed={project === p.id} onclick={() => (project = p.id)}>{p.name}</button>
    {/each}
    <button type="button" class="chip" aria-pressed={project === 'none'} onclick={() => (project = 'none')}>No project</button>
    <span class="gap"></span>
    <button type="button" class="chip" aria-pressed={kind === 'all'} onclick={() => (kind = 'all')}>Everything</button>
    <button type="button" class="chip" aria-pressed={kind === 'ticket'} onclick={() => (kind = 'ticket')}>Tickets</button>
    <button type="button" class="chip" aria-pressed={kind === 'chore'} onclick={() => (kind = 'chore')}>Chores</button>
  </div>

  {#if shown.length}
    <table>
      <thead>
        <tr>
          <th class="tk"><span class="sr">Done</span></th>
          <th>Item</th>
          <th>Kind</th>
          <th>Project</th>
          <th class="num">Time so far</th>
          <th>Last worked</th>
          <th>{app.itemsTab === 'done' ? '' : 'Follow up'}</th>
        </tr>
      </thead>
      <tbody>
        {#each shown as r (r.key)}
          <tr class:now={r.now && !r.done} class:done={r.done}>
            <td class="tk">
              {#if r.ticket}
                <input type="checkbox" checked={r.done} aria-label="{r.name} done" onchange={() => void tick(r)} />
              {:else}
                <Icon name="repeat" size={14} />
              {/if}
            </td>
            <td class="what">
              <button type="button" class="link" onclick={() => openRow(r)} title="Open it">
                <b class:mono={r.kind === 'ticket'}>{r.name}</b>
                {#if r.title}<small>{r.title}</small>{/if}
              </button>
            </td>
            <td>{r.kind === 'ticket' ? 'Ticket' : 'Chore'}</td>
            <td>
              {#if r.home}<span class="proj" style:--c={r.projectId ? projectColor(r.projectId) : 'var(--rep)'}>{r.home}</span>{:else}<span class="mut">No project</span>{/if}
            </td>
            <td class="num mono">{r.minutes ? fmtMinutes(r.minutes) : '–'}</td>
            <td>{r.last ? relativeDay(r.last, app.today) : '–'}</td>
            <td class="fu">
              {#if r.ticket && !r.done}
                <button type="button" class="btn" class:hot={r.now} onclick={(e) => followMenu(e.currentTarget as HTMLElement, r)}>
                  {r.follow ? followWords(r.follow, app.today) : 'Follow up…'}
                </button>
              {:else if r.chore}
                <span class:hot={r.now}>{r.followText}</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty-note">
      {#if rows.length === 0}
        Nothing yet. Tickets you paste or log show here, one line each however many days you work on them, along with your
        repeating chores.
      {:else if app.itemsTab === 'follow'}
        Nothing to follow up. Pick Follow up on a ticket to have it come back on a day.
      {:else if app.itemsTab === 'done'}
        Nothing done here yet.
      {:else}
        Nothing open matches.
      {/if}
    </p>
  {/if}
</section>

<style>
  .items {
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

  .tabs .n {
    font: 600 11px var(--mono);
    margin-left: 4px;
    color: var(--ink-2);
  }

  .grow {
    flex: 1;
  }

  .find {
    flex: 0 1 260px;
    min-width: 0;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }

  .chips .gap {
    width: 12px;
  }

  .chip {
    border: 1px solid var(--rule);
    background: transparent;
    border-radius: 999px;
    padding: 3px 10px;
    font: 600 12.5px var(--ui);
    color: var(--c, var(--ink));
    cursor: pointer;
  }

  .chip[aria-pressed='true'] {
    background: color-mix(in srgb, var(--c, var(--accent)) 14%, transparent);
    border-color: color-mix(in srgb, var(--c, var(--accent)) 45%, transparent);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13.5px;
  }

  th {
    text-align: left;
    font: 700 10.5px var(--ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-2);
    padding: 6px 8px;
    border-bottom: 1px solid var(--rule);
  }

  td {
    padding: 7px 8px;
    border-bottom: 1px solid var(--rule);
    vertical-align: middle;
  }

  .num {
    text-align: right;
    white-space: nowrap;
  }

  .tk {
    width: 28px;
  }

  tr.now td {
    background: color-mix(in srgb, var(--warn) 7%, transparent);
  }

  tr.done .what b {
    text-decoration: line-through;
    color: var(--ink-2);
  }

  .link {
    all: unset;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .link:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .what small {
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }

  .proj {
    color: var(--c);
    font-weight: 600;
  }

  .mut {
    color: var(--ink-2);
  }

  .hot {
    color: var(--warn);
    font-weight: 600;
  }

  .fu .btn {
    white-space: nowrap;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }

  @media (max-width: 760px) {
    th:nth-child(3),
    td:nth-child(3),
    th:nth-child(6),
    td:nth-child(6) {
      display: none;
    }
  }
</style>
