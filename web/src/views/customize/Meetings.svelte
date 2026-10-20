<script lang="ts">
  // Settings → Meetings (round 12): meetings that repeat, so the bell reminds you before they
  // start and opens the day's meeting note. Changes save as you make them.
  import { onMount } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api } from '../../lib/api';
  import type { MeetingSeries, SeriesInput } from '../../lib/api';
  import { refreshBell } from '../../lib/bell.svelte';
  import { attempt } from '../../lib/store.svelte';
  import { toast } from '../../lib/toast.svelte';

  const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const REMIND = [0, 5, 10, 15, 30, 60];

  type Row = SeriesInput & { id: string | null; key: number };
  let rows = $state<Row[]>([]);
  let next = 0;

  const row = (m: MeetingSeries): Row => ({ ...m, days: [...m.days], key: next++ });
  const clock = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
  const minutes = (t: string) => {
    const [h, m] = t.split(':').map(Number);
    return (h ?? 0) * 60 + (m ?? 0);
  };

  onMount(async () => {
    const list = await attempt(() => api.meetingSeries());
    if (list) rows = list.map(row);
  });

  function add() {
    rows.push({ id: null, key: next++, name: '', days: [true, false, false, false, false, false, false], at: 600, remindMinutes: 15 });
  }

  /** Save a row once it has a name and a day; a new one is added the first time. */
  async function save(r: Row) {
    if (!r.name.trim() || !r.days.some(Boolean)) return;
    const input: SeriesInput = { name: r.name, days: r.days, at: r.at, remindMinutes: r.remindMinutes };
    const saved = await attempt(() => api.saveMeetingSeries(r.id, input));
    if (!saved) return;
    r.id = saved.id;
    r.name = saved.name;
    void refreshBell();
  }

  async function remove(r: Row) {
    rows = rows.filter((x) => x !== r);
    if (!r.id) return;
    const id = r.id;
    if (!(await attempt(() => api.setMeetingSeriesDeleted(id, true).then(() => true)))) return;
    void refreshBell();
    toast(`Removed “${r.name}”. Its meeting notes stay.`, 'Undo', () => {
      void attempt(() => api.setMeetingSeriesDeleted(id, false).then(() => true)).then((ok) => {
        if (ok) rows.push({ ...r, key: next++ });
        void refreshBell();
      });
    });
  }
</script>

<article class="paper cz-card">
  <h3>Meetings</h3>
  <p class="sub">
    Meetings that repeat. The bell in the bottom bar reminds you before each one starts and opens
    that day's meeting note. Nothing leaves Shiftbook: there are no pop-ups outside the app.
  </p>
  {#each rows as r (r.key)}
    <div class="series">
      <input
        class="text-in name"
        bind:value={r.name}
        placeholder="Name, e.g. Weekly ops"
        aria-label="Meeting name"
        maxlength="80"
        onchange={() => void save(r)}
      />
      <span class="days" role="group" aria-label="Days for {r.name || 'this meeting'}">
        {#each DAYS as d, i (d)}
          <button
            type="button"
            class="day"
            class:on={r.days[i]}
            aria-pressed={r.days[i]}
            onclick={() => {
              r.days[i] = !r.days[i];
              void save(r);
            }}>{d}</button
          >
        {/each}
      </span>
      <label class="at">
        at
        <input
          class="text-in mono"
          type="time"
          value={clock(r.at)}
          aria-label="Start time"
          onchange={(e) => {
            r.at = minutes((e.currentTarget as HTMLInputElement).value);
            void save(r);
          }}
        />
      </label>
      <label class="at">
        remind
        <select
          class="text-in"
          value={r.remindMinutes}
          aria-label="Remind me before"
          onchange={(e) => {
            r.remindMinutes = Number((e.currentTarget as HTMLSelectElement).value);
            void save(r);
          }}
        >
          {#each REMIND as m (m)}<option value={m}>{m === 0 ? 'at the start' : `${m} min before`}</option>{/each}
        </select>
      </label>
      <button type="button" class="ib" aria-label="Remove {r.name || 'this meeting'}" onclick={() => void remove(r)}>
        <Icon name="trash" size={15} />
      </button>
    </div>
  {:else}
    <p class="empty-note">No meetings yet. Add the ones that repeat, like a weekly team meeting.</p>
  {/each}
  <div class="actions">
    <button class="btn" type="button" onclick={add}><Icon name="plus" size={14} /> Add a meeting</button>
  </div>
</article>

<style>
  .series {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 10px 0;
    border-top: 1px solid var(--rule);
  }

  .name {
    flex: 1 1 200px;
  }

  .days {
    display: inline-flex;
    gap: 3px;
  }

  .day {
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink-2);
    border-radius: 7px;
    padding: 4px 7px;
    font: 600 12px var(--ui);
    cursor: pointer;
  }

  .day.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  .at {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--mut);
  }

  .ib {
    border: 0;
    background: none;
    color: var(--ink-2);
    width: 30px;
    height: 30px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .ib:hover {
    background: color-mix(in srgb, var(--bad) 12%, transparent);
    color: var(--bad);
  }

  .actions {
    padding-top: 8px;
  }
</style>
