<script lang="ts">
  // The bell's panel (round 12): slides in from the right over the work area. Now first
  // (meetings about to start, and since round 15 one that just finished, offering to log its
  // time), then Today (later meetings, things to follow up, each once with Open, Tomorrow and
  // Done, repeating things that need doing, tickets still open). Each line has the button that does
  // the obvious thing. Click outside or press Esc to close it.
  // After a day or more away it opens by itself as "While you were away" (round 15), on just
  // what came due meanwhile; Got it shows the usual panel.
  import { fly } from 'svelte/transition';
  import { api } from '../lib/api';
  import type { BellItem, CardKind } from '../lib/api';
  import { bell, nowMinutes, refreshBell, snoozeHour, snoozeSheet } from '../lib/bell.svelte';
  import { addDays, fmtMinutes, relativeDay } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { bringUp } from '../lib/bringUp';
  import { markDone, openRepeating } from '../lib/repeating.svelte';
  import { app, attempt, flashEntry, projectName, refresh, selectDay, setView, timeButtons, words } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { openWindow } from '../lib/windows.svelte';
  import Icon from './Icon.svelte';

  const away = $derived(bell.away);
  const now = $derived(away ? [] : bell.items.filter((i) => i.when === 'now'));
  const today = $derived(away ? away.items : bell.items.filter((i) => i.when === 'today'));
  const ms = $derived((app.prefs?.motion === 'still' ? 0 : 1) * 320);

  function close() {
    bell.open = false;
    bell.away = null;
  }

  /** "Got it" on While you were away: the usual panel. */
  function seen() {
    bell.away = null;
  }

  async function dueDone(id: string, title: string) {
    await markDone(id, title.split(':')[0] ?? title);
    if (bell.away) bell.away.items = bell.away.items.filter((i) => !(i.about.kind === 'due' && i.about.id === id));
  }

  function openDue(id: string, renew: boolean) {
    openRepeating(id, renew);
    close();
  }

  async function openMeetingNote(series: string) {
    const note = await attempt(() => api.meetingNote(series, app.today));
    if (!note) return;
    openWindow('note', note.id);
    close();
  }

  async function logMeeting(series: string, title: string, minutes: number) {
    const day = app.today;
    const entry = await attempt(() => api.logMeeting(series, day, minutes));
    if (!entry) return;
    await Promise.all([refresh(), refreshBell()]);
    flashEntry(entry.id);
    toast(
      `Logged ${fmtMinutes(minutes)} of ${title}`,
      'Undo',
      () => void attempt(() => api.deleteEntry(entry.id).then(() => api.skipMeeting(series, day, false))).then(() => Promise.all([refresh(), refreshBell()])),
      () => void attempt(() => api.restoreEntry(entry.id).then(() => api.skipMeeting(series, day, true))).then(() => Promise.all([refresh(), refreshBell()])),
    );
  }

  function otherTime(e: MouseEvent, series: string, title: string) {
    openMenu(e.currentTarget as HTMLElement, {
      sections: [
        {
          head: 'How long was it?',
          choices: timeButtons().map((m) => ({ label: fmtMinutes(m), color: null, checked: false, act: () => void logMeeting(series, title, m) })),
        },
      ],
    });
  }

  async function skipMeeting(series: string) {
    const day = app.today;
    if (!(await attempt(() => api.skipMeeting(series, day, true).then(() => true)))) return;
    await refreshBell();
    toast('Skipped. Nothing logged.', 'Undo', () => void attempt(() => api.skipMeeting(series, day, false)).then(refreshBell));
  }

  type Follow = { card: string | null; nextUp: string | null };

  function cardOf(f: Follow): [CardKind, string] | null {
    return f.card ? (f.card.split(':') as [CardKind, string]) : null;
  }

  /** Done: it's followed up, so it leaves the bell (a pin stays on the bar). */
  async function followDone(f: Follow) {
    const card = cardOf(f);
    const ok = await attempt(() =>
      Promise.all([card ? api.clearFollowUp(card[0], card[1]) : null, f.nextUp ? api.tickNextUp(f.nextUp, true) : null]).then(() => true),
    );
    if (ok) await refreshBell();
  }

  function followOpen(f: Follow) {
    const card = cardOf(f);
    if (!card) return;
    openWindow(card[0], card[1]);
    close();
  }

  async function followTomorrow(f: Follow, title: string) {
    const card = cardOf(f);
    if (card) await bringUp(card[0], card[1], title, app.bringUpDays[0] ?? addDays(app.today, 1));
  }

  const KIND_WORD: Record<CardKind, string> = { entry: 'Work', tray: 'Ticket', note: 'Note', inbox: 'Jot' };

  async function openSheet() {
    await selectDay(app.today);
    setView('timesheet');
    close();
  }

  const clockText = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:00`;

  function showDay() {
    setView('day');
    close();
  }

  function meetings() {
    app.settingsSection = 'meetings';
    setView('settings');
    close();
  }

  function detail(i: BellItem): string {
    switch (i.about.kind) {
      case 'timesheet': {
        const n = i.about.uncoded;
        return `${i.about.hours} h logged${n ? ` · ${n} ${n === 1 ? 'entry needs' : 'entries need'} a code` : ''}`;
      }
      case 'meeting_done':
        return i.about.project
          ? `Just finished · logs to ${projectName(i.about.project)}, with the meeting note linked`
          : 'Just finished · logs your time, with the meeting note linked';
      case 'meeting':
        return i.about.state.state === 'later' ? 'Your meeting note opens from here' : 'Open your meeting note';
      case 'follow_up': {
        const card = cardOf(i.about);
        const what = card ? `${KIND_WORD[card[0]]} · ` : '';
        return what + (i.about.on === app.today ? 'follow up today' : `follow up since ${relativeDay(i.about.on, app.today)}`);
      }
      case 'due':
        return i.about.state === 'renewed' ? 'Renewed, but the expiry date is still the old one' : `Repeating · due ${relativeDay(i.about.nextDue, app.today)}`;
      case 'still_open':
        return i.about.tickets.join(' · ');
    }
    return '';
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (bell.open && e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }}
/>

{#if bell.open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="catch" onclick={close}></div>
  <aside class="panel" aria-label="Coming up" transition:fly={{ x: 380, duration: ms, opacity: 1 }}>
    <header>
      <h2>{away ? 'While you were away' : 'Coming up'}</h2>
      <button type="button" class="x" aria-label="Close" onclick={close}><Icon name="x" size={16} /></button>
    </header>
    {#if away}
      <p class="since">
        Things that came due since you last opened Shiftbook{away.since ? `, ${relativeDay(away.since, app.today).toLowerCase()}` : ''}.
      </p>
    {/if}
    {#each [['Now', now], [away ? 'Since then' : 'Today', today]] as [head, list] (head)}
      {#if (list as BellItem[]).length}
        <h3>{head}</h3>
        <ul>
          {#each list as BellItem[] as i, n (i.title + n)}
            <li class="item {i.about.kind}" class:soon={i.when === 'now'} class:late={i.about.kind === 'due' && i.about.state === 'overdue'} style:--i={n}>
              <span class="ico" aria-hidden="true">
                <Icon
                  name={i.about.kind === 'timesheet' ? 'sheet' : i.about.kind === 'meeting' || i.about.kind === 'meeting_done' ? 'meeting' : i.about.kind === 'follow_up' ? 'sun' : i.about.kind === 'due' ? 'repeat' : 'clock'}
                  size={16}
                />
              </span>
              <span class="txt"><b>{i.title}</b><small>{detail(i)}</small></span>
              <span class="btns">
                {#if i.about.kind === 'timesheet'}
                  <button type="button" class="btn primary" onclick={() => void openSheet()}>Open the crib sheet</button>
                  <button type="button" class="btn" onclick={snoozeSheet}>Remind me at {clockText(snoozeHour(nowMinutes()))}</button>
                {:else if i.about.kind === 'meeting_done'}
                  {@const { series, name, minutes } = i.about}
                  <button type="button" class="btn primary" onclick={() => void logMeeting(series, name, minutes)}>Log {fmtMinutes(minutes)}</button>
                  <button type="button" class="btn" onclick={(e) => otherTime(e, series, name)}>Other time…</button>
                  <button type="button" class="btn" onclick={() => void skipMeeting(series)}>Skip</button>
                {:else if i.about.kind === 'meeting'}
                  {@const series = i.about.series}
                  <button type="button" class="btn primary" onclick={() => void openMeetingNote(series)}>Open note</button>
                {:else if i.about.kind === 'follow_up'}
                  {@const f = i.about}
                  {@const title = i.title}
                  {#if f.card}
                    <button type="button" class="btn primary" onclick={() => followOpen(f)}>Open</button>
                    <button type="button" class="btn" title="Follow it up tomorrow" onclick={() => void followTomorrow(f, title)}>Tomorrow</button>
                  {/if}
                  <button type="button" class="btn" onclick={() => void followDone(f)}><Icon name="check" size={13} /> Done</button>
                {:else if i.about.kind === 'due'}
                  {@const id = i.about.id}
                  {@const title = i.title}
                  {#if i.about.state === 'renewed'}
                    <button type="button" class="btn primary" onclick={() => openDue(id, true)}>New date…</button>
                  {:else}
                    <button type="button" class="btn primary" onclick={() => void dueDone(id, title)}><Icon name="check" size={13} /> Done</button>
                    <button type="button" class="btn" onclick={() => openDue(id, false)}>Open</button>
                  {/if}
                {:else}
                  <button type="button" class="btn" onclick={showDay}>Show</button>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/each}
    {#if away}
      <div class="away-btns">
        <button type="button" class="btn" onclick={close}>Later</button>
        <button type="button" class="btn primary" onclick={seen}>Got it</button>
      </div>
    {:else}
      {#if bell.items.length === 0}
        <p class="empty">
          Nothing coming up. {words().meetings} you add show here before they start, along with repeating things that are due,
          things to follow up today and {words().tickets.toLowerCase()} still open.
        </p>
      {/if}
      <button type="button" class="link" onclick={meetings}><Icon name="meeting" size={14} /> Your {words().meetings.toLowerCase()}</button>
    {/if}
  </aside>
{/if}

<style>
  .catch {
    position: absolute;
    inset: 0;
    z-index: 43;
  }

  .panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(380px, 100%);
    z-index: 44;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 16px 14px;
    overflow: auto;
    background: color-mix(in srgb, var(--desk) 92%, var(--paper));
    border-left: 1px solid var(--rule);
    box-shadow: -30px 0 60px -30px rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(14px);
  }

  header {
    display: flex;
    align-items: center;
  }

  h2 {
    margin: 0;
    font: 700 20px var(--display);
    flex: 1;
  }

  h3 {
    margin: 6px 0 0;
    font: 700 10px var(--ui);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--mut);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .item {
    display: grid;
    grid-template-columns: 26px minmax(0, 1fr);
    gap: 4px 8px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--paper);
    box-shadow: var(--shadow);
    border-left: 3px solid var(--accent);
    animation: in calc(0.3s * var(--m)) var(--ease) both;
    animation-delay: calc(var(--i) * 40ms * var(--m));
  }

  @keyframes in {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
  }

  .item.meeting {
    border-left-color: var(--warn);
  }

  .item.due {
    border-left-color: var(--warn);
  }

  .item.late {
    border-left-color: var(--bad);
  }

  .since {
    margin: -4px 0 0;
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .away-btns {
    margin-top: auto;
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .item.still_open {
    border-left-color: color-mix(in srgb, var(--warn) 70%, var(--bad));
  }

  .item.soon {
    border-left-color: var(--bad);
  }

  .ico {
    color: var(--ink-2);
    padding-top: 1px;
  }

  .txt {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .txt b {
    font-size: 13.5px;
  }

  .txt small {
    font-size: 12px;
    color: var(--mut);
    overflow-wrap: anywhere;
  }

  .btns {
    grid-column: 2;
    display: flex;
    gap: 6px;
  }

  .btns .btn {
    padding: 3px 10px;
    font-size: 12px;
  }

  .x {
    border: 0;
    background: none;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .x:hover {
    background: color-mix(in srgb, var(--ink) 8%, transparent);
  }

  .empty {
    color: var(--mut);
    font-size: 13px;
  }

  .link {
    margin-top: auto;
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--accent);
    font: 600 12.5px var(--ui);
    cursor: pointer;
    padding: 4px 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .item {
      animation: none;
    }
  }
</style>
