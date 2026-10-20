<script lang="ts">
  // The bell's panel (round 12): slides in from the right over the work area. Now first
  // (meetings about to start), then Today (later meetings, things brought up for today,
  // tickets still open). Each line has the button that does the obvious thing. Click outside
  // or press Esc to close it.
  import { fly } from 'svelte/transition';
  import { api } from '../lib/api';
  import type { BellItem } from '../lib/api';
  import { bell, refreshBell } from '../lib/bell.svelte';
  import { app, attempt, setView } from '../lib/store.svelte';
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

  async function done(id: string) {
    if (await attempt(() => api.tickNextUp(id, true).then(() => true))) await refreshBell();
  }

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
      case 'meeting':
        return i.about.state.state === 'later' ? 'Your meeting note opens from here' : 'Open your meeting note';
      case 'brought_up':
        return 'You brought this up for today';
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
                <Icon name={i.about.kind === 'meeting' ? 'meeting' : i.about.kind === 'brought_up' ? 'sun' : 'clock'} size={16} />
              </span>
              <span class="txt"><b>{i.title}</b><small>{detail(i)}</small></span>
              <span class="btns">
                {#if i.about.kind === 'meeting'}
                  {@const series = i.about.series}
                  <button type="button" class="btn primary" onclick={() => void openMeetingNote(series)}>Open note</button>
                {:else if i.about.kind === 'brought_up'}
                  {@const id = i.about.id}
                  <button type="button" class="btn" onclick={() => void done(id)}>Done</button>
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
