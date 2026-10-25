<script lang="ts">
  // Repeating things (round 15): bills, bins, filters, renewals, birthdays, and at work
  // certificates and patching. Sorted by what needs doing first; one click marks a thing done
  // and the next date is worked out (crates/core/src/recurring.rs). Things that expire get
  // "Renewed…" instead, which asks for the new expiry date. The form on the right adds one or
  // changes the one picked.
  import { onMount, untrack } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import type { Frequency, Recurring, RecurringInput } from '../lib/api';
  import { refreshBell } from '../lib/bell.svelte';
  import { addDays, dayMonth, longDay } from '../lib/format';
  import { markDone, markRenewed, refreshRepeating, repeating } from '../lib/repeating.svelte';
  import { app, attempt, refresh } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  const WARN = [
    { days: 0, label: 'On the day' },
    { days: 1, label: '1 day' },
    { days: 7, label: '1 week' },
    { days: 30, label: '1 month' },
    { days: 90, label: '3 months' },
    { days: 180, label: '6 months' },
  ];

  const GROUPS: { head: string; tone: string; states: Recurring['state'][] }[] = [
    { head: 'Overdue', tone: 'bad', states: ['overdue'] },
    { head: 'Put in the new expiry date', tone: 'warn', states: ['renewed'] },
    { head: 'Due soon', tone: 'warn', states: ['due_soon'] },
    { head: 'Needs a date', tone: 'warn', states: ['needs_date'] },
    { head: 'Fine for now', tone: 'ok', states: ['ok'] },
    { head: 'Routine (no reminders)', tone: 'none', states: ['ongoing', 'inactive'] },
  ];

  let frequencies = $state<Frequency[]>([]);
  let kinds = $state<string[]>([]);

  type Form = {
    id: string | null;
    name: string;
    kind: string;
    expires: boolean;
    frequencyId: string;
    date: string;
    warnDays: number;
    alerts: boolean;
    notes: string;
  };

  function blank(): Form {
    const weekly = frequencies.find((f) => f.label === 'Weekly');
    return {
      id: null,
      name: '',
      kind: '',
      expires: false,
      frequencyId: weekly?.id ?? frequencies[0]?.id ?? '',
      date: addDays(app.today, 7),
      warnDays: 1,
      alerts: true,
      notes: '',
    };
  }

  let form = $state<Form>({ id: null, name: '', kind: '', expires: false, frequencyId: '', date: '', warnDays: 1, alerts: true, notes: '' });
  let renewing = $state<{ id: string; name: string; date: string } | null>(null);
  let nameEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    const choices = await attempt(() => api.recurringChoices());
    if (choices) {
      frequencies = choices.frequencies;
      kinds = choices.kinds;
    }
    form = blank();
    await refreshRepeating();
  });

  // Opened from the bell or the Desk on one thing.
  $effect(() => {
    void repeating.open.serial;
    const { id, renew } = repeating.open;
    const r = repeating.items.find((x) => x.id === id);
    if (!r) return;
    untrack(() => (renew && r.expires ? startRenew(r) : edit(r)));
  });

  function edit(r: Recurring) {
    renewing = null;
    form = {
      id: r.id,
      name: r.name,
      kind: r.kind ?? '',
      expires: r.expires,
      frequencyId: r.frequencyId ?? '',
      date: (r.expires ? r.expirationDate : r.nextDue) ?? '',
      warnDays: r.warnDays,
      alerts: r.alerts,
      notes: r.notes,
    };
    nameEl?.focus();
  }

  function startRenew(r: Recurring) {
    const after = r.expirationDate && r.expirationDate > app.today ? r.expirationDate : app.today;
    // A year on is the usual answer; the frequency, if set, gives a better guess in the box.
    renewing = { id: r.id, name: r.name, date: r.state === 'renewed' && r.nextDue ? r.nextDue : addDays(after, 365) };
  }

  async function save() {
    const input: RecurringInput = {
      name: form.name,
      kind: form.kind.trim() || null,
      frequencyId: form.frequencyId || null,
      expires: form.expires,
      date: form.date || null,
      warnDays: form.warnDays,
      alerts: form.alerts,
      notes: form.notes,
    };
    if (!form.name.trim()) {
      toast('Give it a name first.');
      return;
    }
    if (form.expires ? !form.date : !form.frequencyId) {
      toast(form.expires ? 'When does it expire?' : 'How often does it come round?');
      return;
    }
    const ok = await attempt(() => api.saveRecurring(form.id, input).then(() => true));
    if (!ok) return;
    toast(form.id ? `Saved “${form.name.trim()}”.` : `Added “${form.name.trim()}”.`);
    form = blank();
    await Promise.all([refreshRepeating(), refreshBell()]);
  }

  async function renew() {
    if (!renewing) return;
    if (renewing.date <= app.today) {
      toast('The new expiry date needs to be after today.');
      return;
    }
    if (await markRenewed(renewing.id, renewing.name, renewing.date)) renewing = null;
  }

  async function remove(r: Recurring) {
    const ok = await attempt(() => api.setRecurringDeleted(r.id, true).then(() => true));
    if (!ok) return;
    if (form.id === r.id) form = blank();
    await Promise.all([refreshRepeating(), refreshBell()]);
    toast(`Removed “${r.name}”. Days you ticked it stay.`, 'Undo', () => {
      void attempt(() => api.setRecurringDeleted(r.id, false).then(() => true)).then((back) => {
        if (back) void Promise.all([refreshRepeating(), refreshBell(), refresh()]);
      });
    });
  }

  function when(r: Recurring): string {
    if (r.state === 'ongoing') return r.every ?? 'Collects time';
    if (!r.nextDue) return 'no date yet';
    if (r.state === 'renewed') return `next about ${dayMonth(r.nextDue)}`;
    return r.dueWords ?? '';
  }

  function detail(r: Recurring): string {
    const parts: string[] = [];
    if (r.expires && r.expirationDate) parts.push(`Expires ${longDay(r.expirationDate)}`);
    else if (r.every) parts.push(r.every);
    if (r.lastDone) parts.push(`last done ${dayMonth(r.lastDone)}`);
    if (r.kind) parts.push(r.kind);
    return parts.join(' · ');
  }
</script>

<div class="rep">
  <section class="paper list" aria-label="Repeating things">
    <header>
      <h1>Repeating things</h1>
      <button class="btn primary" type="button" onclick={() => ((form = blank()), (renewing = null), nameEl?.focus())}>
        <Icon name="plus" size={14} /> Add
      </button>
    </header>
    <p class="sub">
      Bills, bins, filters, renewals and birthdays. Overdue and due soon ones also show on the Desk and in the bell.
    </p>
    {#each GROUPS as g (g.head)}
      {@const items = repeating.items.filter((r) => g.states.includes(r.state))}
      {#if items.length}
        <h2 class="grp {g.tone}">{g.head}</h2>
        <ul>
          {#each items as r (r.id)}
            <li class="row {g.tone}" class:picked={form.id === r.id}>
              <span class="bar" aria-hidden="true"></span>
              <button type="button" class="what" onclick={() => edit(r)} title="Change it">
                <b>{r.name}</b>
                <small>{detail(r)}</small>
              </button>
              <span class="when {g.tone}">{when(r)}</span>
              <span class="acts">
                {#if r.expires}
                  <button type="button" class="btn" onclick={() => startRenew(r)}>Renewed…</button>
                {:else}
                  <button type="button" class="btn" onclick={() => void markDone(r.id, r.name)}><Icon name="check" size={14} /> Done</button>
                {/if}
                <button type="button" class="ib" aria-label="Remove {r.name}" onclick={() => void remove(r)}><Icon name="trash" size={15} /></button>
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/each}
    {#if repeating.items.length === 0}
      <p class="empty-note">
        Nothing yet. Add the things that come round again, like putting the bins out every Thursday, a bill each month or a
        passport that expires.
      </p>
    {/if}
  </section>

  <div class="side">
    {#if renewing}
      <section class="paper card" aria-label="Renewed">
        <span class="lbl">Renewed…</span>
        <p class="ask">{renewing.name} renewed. When does the new one run out?</p>
        <div class="in">
          <input class="text-in" type="date" bind:value={renewing.date} min={addDays(app.today, 1)} aria-label="New expiry date" />
          <button type="button" class="btn primary" onclick={() => void renew()}>Save</button>
          <button type="button" class="btn" onclick={() => (renewing = null)}>Cancel</button>
        </div>
      </section>
    {/if}

    <section class="paper card" aria-label={form.id ? 'Change it' : 'New repeating thing'}>
      <h2>{form.id ? 'Change it' : 'New repeating thing'}</h2>
      <div class="form">
        <label for="rep-name">What is it?</label>
        <input id="rep-name" class="text-in" bind:this={nameEl} bind:value={form.name} placeholder="e.g. Bins out" maxlength="200" />

        <span class="lab">It…</span>
        <span class="seg" role="group" aria-label="Repeats or expires">
          <button type="button" aria-pressed={!form.expires} onclick={() => (form.expires = false)}>repeats</button>
          <button type="button" aria-pressed={form.expires} onclick={() => (form.expires = true)}>expires on a date</button>
        </span>

        {#if !form.expires}
          <label for="rep-every">Every</label>
          <select id="rep-every" class="text-in" bind:value={form.frequencyId}>
            {#each frequencies as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
          </select>
          <label for="rep-date">Next time</label>
          <input id="rep-date" class="text-in" type="date" bind:value={form.date} />
        {:else}
          <label for="rep-date">Expires on</label>
          <input id="rep-date" class="text-in" type="date" bind:value={form.date} />
          <label for="rep-every">Renew every</label>
          <select id="rep-every" class="text-in" bind:value={form.frequencyId}>
            <option value="">Not sure</option>
            {#each frequencies as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
          </select>
        {/if}

        <span class="lab">Warn me</span>
        <span class="seg" role="group" aria-label="Warn me before">
          {#each WARN as w (w.days)}
            <button type="button" aria-pressed={form.warnDays === w.days} onclick={() => (form.warnDays = w.days)}>{w.label}</button>
          {/each}
        </span>

        <label for="rep-kind">Kind</label>
        <input id="rep-kind" class="text-in" list="rep-kinds" bind:value={form.kind} placeholder="e.g. Bills" maxlength="60" />
        <datalist id="rep-kinds">{#each kinds as k (k)}<option value={k}></option>{/each}</datalist>

        <label for="rep-notes">Notes</label>
        <input id="rep-notes" class="text-in" bind:value={form.notes} placeholder="e.g. policy number, who to ring" />
      </div>
      <div class="foot">
        <label class="check-row" title="Turn off for routine things that only collect time">
          <input type="checkbox" bind:checked={form.alerts} /> Remind me
        </label>
        <span class="grow"></span>
        {#if form.id}<button type="button" class="btn" onclick={() => (form = blank())}>Cancel</button>{/if}
        <button type="button" class="btn primary" onclick={() => void save()}>Save</button>
      </div>
      <p class="note">
        {form.expires
          ? 'When it’s renewed, press Renewed… and put in the new expiry date.'
          : 'Press Done when you’ve done it; the next time is worked out from that day.'}
      </p>
    </section>
  </div>
</div>

<style>
  .rep {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(300px, 1fr);
    gap: 14px;
    align-items: start;
    max-width: 1180px;
    margin: 0 auto;
  }

  @media (max-width: 900px) {
    .rep {
      grid-template-columns: 1fr;
    }
  }

  .list,
  .card {
    padding: 16px 18px;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  h1 {
    margin: 0;
    font: 650 24px var(--display);
  }

  h2 {
    margin: 0 0 10px;
    font: 650 17px var(--display);
  }

  .sub {
    margin: 4px 0 6px;
    color: var(--mut);
    font-size: 13px;
  }

  .grp {
    margin: 14px 0 4px;
    font: 700 11px var(--mono);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--c);
  }

  .bad {
    --c: var(--bad);
  }
  .warn {
    --c: var(--warn);
  }
  .ok {
    --c: var(--ok);
  }
  .none {
    --c: var(--mut);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .row {
    display: grid;
    grid-template-columns: 4px minmax(0, 1fr) auto auto;
    gap: 10px;
    align-items: center;
    padding: 7px 4px;
    border-bottom: 1px solid var(--rule);
    border-radius: 6px;
  }

  .row.picked {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .bar {
    align-self: stretch;
    border-radius: 2px;
    background: var(--c);
  }

  .what {
    border: 0;
    background: none;
    text-align: left;
    padding: 2px 0;
    cursor: pointer;
    min-width: 0;
    color: inherit;
    font: inherit;
  }

  .what b {
    display: block;
    font-weight: 600;
    font-size: 14.5px;
  }

  .what small {
    color: var(--mut);
    font-size: 12.5px;
  }

  .when {
    font: 700 12.5px var(--mono);
    color: var(--c);
    white-space: nowrap;
  }

  .acts {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }

  .ib {
    border: 0;
    background: none;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--mut);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .ib:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
    color: var(--bad);
  }

  .form {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 9px 10px;
    align-items: center;
  }

  .form label,
  .lab {
    font-size: 13px;
    color: var(--ink-2);
  }

  .form .seg {
    justify-self: start;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }

  .grow {
    flex: 1;
  }

  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--mut);
  }

  .ask {
    margin: 4px 0 10px;
    color: var(--ink-2);
  }

  .in {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
