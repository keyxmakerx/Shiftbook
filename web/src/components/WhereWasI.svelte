<script lang="ts">
  // "Where was I?": after an unlock, a break or a new day, the Desk offers a way back in.
  // Things brought up for today and the wrap-up list come first (the first item, then the
  // rest as a checklist); otherwise the last entry with its tickets and notepad. When it shows and what it holds is
  // decided by the app (crates/core/src/resume.rs).
  import { api } from '../lib/api';
  import { bell } from '../lib/bell.svelte';
  import { fmtMinutes } from '../lib/format';
  import { afterNow, cardLead, cardTitle, carryOnText, lastLogged, startedAt } from '../lib/resume';
  import { copyStandup } from '../lib/standup';
  import { app, attempt, focusLog, logNow, projectById, projectColor, timeButtons } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import TicketChip from './TicketChip.svelte';

  // Carry on turns the card into "Now" (round 21): it stays, with the rest of the list, until
  // the thing is logged or you switch.
  const now = $derived(app.now);
  const card = $derived(app.now?.card ?? app.resume);
  const entry = $derived(card?.entry ?? null);
  const project = $derived(projectById(entry?.projectId));
  const color = $derived(projectColor(entry?.projectId ?? null));
  const rest = $derived(card?.list.slice(1) ?? []);
  // While you were away rides along here instead of opening the bell too (round 18).
  const awayCount = $derived(bell.away?.items.length ?? 0);

  function carryOn() {
    if (!card) return;
    app.now = { card, startedAt: Date.now() };
    app.resume = null;
  }

  /** Once the Now card's thing is logged: tick it off the list and move on to the next. */
  function logged(on: NonNullable<typeof app.now>) {
    return () => {
      if (app.now !== on) return;
      const first = on.card.entry ? null : on.card.list[0];
      if (first) void attempt(() => api.tickNextUp(first.id, true));
      app.now = null;
      app.resume = afterNow(on.card);
    };
  }

  /** A time button on the Now card: log it through Log work. */
  function logIt(minutes: number) {
    const on = app.now;
    if (on) logNow(carryOnText(on.card), minutes, logged(on));
  }

  /** Other time: the line goes to Log work to type or pick any time; the card stays until
   *  that is logged. */
  function otherTime() {
    const on = app.now;
    if (on) focusLog(carryOnText(on.card), null, logged(on));
  }

  /** Switch: back to "Where was I?" with the whole list. */
  function switchAway() {
    if (!app.now) return;
    app.resume = app.now.card;
    app.now = null;
  }

  function somethingElse() {
    app.resume = null;
    app.now = null;
    focusLog();
  }

  function later() {
    app.resume = null;
    app.now = null;
    app.resumeLater = true;
    toast("It's back when you next unlock.");
  }

  async function tick(id: string, done: boolean) {
    if (!card) return;
    await attempt(() => api.tickNextUp(id, done));
    const item = card.list.find((i) => i.id === id);
    if (item) item.done = done;
  }
</script>

{#if card}
  <section class="resume" class:now={!!now} style:--c={entry ? color : 'var(--accent)'} aria-labelledby="resume-h">
    <div class="top">
      {#if now}
        <span class="lbl">Now</span>
        <span class="ago">started {startedAt(now.startedAt)}</span>
      {:else}
        <span class="lbl">Where was I? · {cardLead(card)}</span>
        <span class="ago">{entry ? lastLogged(card.changedAt, new Date()) : card.list[0]?.from ? 'You brought this up for today' : 'From your wrap-up'}</span>
      {/if}
    </div>
    <h3 id="resume-h">{cardTitle(card)}</h3>

    {#if entry}
      {#if project}<span class="proj"><span class="dot" aria-hidden="true"></span>{project.name}</span>{/if}
      {#if entry.tickets.length}
        <div class="chips">
          {#each entry.tickets as t (t.id)}<TicketChip number={t.number} links={t.links} {color} />{/each}
        </div>
      {/if}
      {#if entry.notes}<p class="next"><b>Notepad:</b> {entry.notes.split('\n')[0]}</p>{/if}
    {/if}

    {#if now}
      <div class="acts">
        <span class="lbl done-lbl">Done · log it</span>
        {#each timeButtons() as m (m)}
          <button class="btn" type="button" onclick={() => logIt(m)}>{fmtMinutes(m)}</button>
        {/each}
        <button class="btn" type="button" title="Put it in Log work to type a time" onclick={otherTime}>Other time…</button>
        <button class="btn" type="button" title="Back to Where was I?" onclick={switchAway}>Switch</button>
      </div>
    {/if}

    {#if !entry && rest.length}
      {#if now}<span class="lbl">Then</span>{/if}
      <ul class="then" aria-label="Then">
        {#each rest as item (item.id)}
          <li>
            <label class:done={item.done}>
              <input type="checkbox" checked={item.done} onchange={(e) => tick(item.id, e.currentTarget.checked)} />
              {item.text}
            </label>
          </li>
        {/each}
      </ul>
    {/if}

    {#if awayCount && !now}
      <button type="button" class="away" onclick={() => (bell.open = true)}>
        While you were away: {awayCount} {awayCount === 1 ? 'thing' : 'things'} came due. See them
      </button>
    {/if}

    {#if !now}
    <div class="acts">
      <button class="btn primary" type="button" onclick={carryOn}>Carry on</button>
      <button class="btn" type="button" onclick={somethingElse}>Something else</button>
      <button class="btn" type="button" onclick={later}>Later</button>
      <button class="btn standup" type="button" title="Copies what you did on your last day and what you start with" onclick={() => void copyStandup()}>
        Copy for stand-up
      </button>
    </div>
    {/if}
  </section>
{/if}

<style>
  .away {
    align-self: flex-start;
    border: 0;
    background: none;
    padding: 0;
    font: 600 12.5px var(--ui);
    color: var(--warn);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .resume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 10px;
    border-left: 4px solid var(--c);
    background: color-mix(in srgb, var(--c) 7%, var(--paper));
    animation: fold 0.35s ease-out;
  }

  @keyframes fold {
    from {
      opacity: 0;
      transform: translateY(-8px) scaleY(0.96);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .top {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }

  .ago {
    color: var(--mut);
    font-size: 12.5px;
  }

  h3 {
    margin: 2px 0 0;
    font: 600 19px/1.25 var(--display);
  }

  .proj {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--ink-2);
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
  }

  .chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .next {
    margin: 0;
  }

  .then {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .then label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .then .done {
    color: var(--mut);
    text-decoration: line-through;
  }

  .acts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
    margin-top: 4px;
  }

  .resume.now {
    background: color-mix(in srgb, var(--c) 12%, var(--paper));
  }

  .done-lbl {
    margin-right: 2px;
  }
</style>
