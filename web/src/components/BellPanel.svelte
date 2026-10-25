<script lang="ts">
  // The bell's panel (round 12): slides in from the right over the work area. Now first
  // (meetings about to start, and since round 15 one that just finished, offering to log its
  // time), then Today (later meetings, things brought up for today, pins
  // due to chase, tickets still open). Each line has the button that does the obvious thing. Click outside
  // or press Esc to close it.
  import { fly } from 'svelte/transition';
  import { api } from '../lib/api';
  import type { BellItem, CardKind } from '../lib/api';
  import { bell, nowMinutes, refreshBell, snoozeHour, snoozeSheet } from '../lib/bell.svelte';
  import { addDays, fmtMinutes, relativeDay } from '../lib/format';
  import { openMenu } from '../lib/menu.svelte';
  import { chaseOn } from '../lib/pins.svelte';
  import { app, attempt, flashEntry, projectName, refresh, selectDay, setView, timeButtons } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { openWindow } from '../lib/windows.svelte';
  import Icon from './Icon.svelte';

  const now = $derived(bell.items.filter((i) => i.when === 'now'));
  const today = $derived(bell.items.filter((i) => i.when === 'today'));
  const ms = $derived((app.prefs?.motion === 'still' ? 0 : 1) * 320);

  function close() {
    bell.open = false;
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

  async function done(id: string) {
    if (await attempt(() => api.tickNextUp(id, true).then(() => true))) await refreshBell();
  }

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
      case 'brought_up':
        return 'You brought this up for today';
      case 'chase':
        return i.about.on === app.today ? 'Pinned · chase it today' : `Pinned · to chase since ${relativeDay(i.about.on, app.today)}`;
      case 'still_open':
        return i.about.tickets.join(' · ');
    }
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
      <h2>Coming up</h2>
      <button type="button" class="x" aria-label="Close" onclick={close}><Icon name="x" size={16} /></button>
    </header>
    {#each [['Now', now], ['Today', today]] as [head, list] (head)}
      {#if (list as BellItem[]).length}
        <h3>{head}</h3>
        <ul>
          {#each list as BellItem[] as i, n (i.title + n)}
            <li class="item {i.about.kind}" class:soon={i.when === 'now'} style:--i={n}>
              <span class="ico" aria-hidden="true">
                <Icon name={i.about.kind === 'timesheet' ? 'sheet' : i.about.kind === 'meeting' || i.about.kind === 'meeting_done' ? 'meeting' : i.about.kind === 'brought_up' ? 'sun' : i.about.kind === 'chase' ? 'pin' : 'clock'} size={16} />
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
                {:else if i.about.kind === 'brought_up'}
                  {@const id = i.about.id}
                  <button type="button" class="btn" onclick={() => void done(id)}>Done</button>
                {:else if i.about.kind === 'chase'}
                  {@const [kind, id] = i.about.card.split(':') as [CardKind, string]}
                  <button type="button" class="btn primary" onclick={() => openWindow(kind, id)}>Open</button>
                  <button type="button" class="btn" title="Chase it again tomorrow" onclick={() => void chaseOn(kind, id, addDays(app.today, 1))}>Tomorrow</button>
                {:else}
                  <button type="button" class="btn" onclick={showDay}>Show</button>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/each}
    {#if bell.items.length === 0}
      <p class="empty">Nothing coming up. Meetings you add show here before they start, along with things you brought up for today and tickets still open.</p>
    {/if}
    <button type="button" class="link" onclick={meetings}><Icon name="meeting" size={14} /> Your meetings</button>
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
