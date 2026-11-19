<script lang="ts">
  // Settings → Notes: auto-archive (r7-notes card). Jot, Daily and Meeting notes move to
  // the archive after this many days without a change; Pages and MOPs never do. Each
  // note warns a week before it moves, with a Keep button.
  import { app, savePref } from '../../lib/store.svelte';

  const CHOICES: { days: number; label: string }[] = [
    { days: 0, label: 'Never (off)' },
    { days: 30, label: 'After 1 month' },
    { days: 60, label: 'After 2 months' },
    { days: 90, label: 'After 3 months' },
    { days: 180, label: 'After 6 months' },
    { days: 365, label: 'After a year' },
  ];

  const prefs = $derived(app.prefs);
</script>

{#if prefs}
  <article class="paper cz-card">
    <h3>Archive old notes on their own</h3>
    <p class="sub">
      Jot, Daily and Meeting notes that haven't changed for a while move to the archive, out of
      the way. Search still finds them, and nothing is deleted. A week before a note moves it
      says so, with a Keep button. Pages and MOPs never move on their own.
    </p>
    <label class="cz-row">
      <span>Move notes</span>
      <select
        class="text-in"
        value={String(prefs.notesAutoArchiveDays)}
        onchange={(e) => savePref('notes.auto_archive_days', (e.currentTarget as HTMLSelectElement).value)}
      >
        {#each CHOICES as c (c.days)}<option value={String(c.days)}>{c.label}</option>{/each}
      </select>
    </label>
  </article>
  <article class="paper cz-card">
    <h3>What today's note starts with</h3>
    <p class="sub">
      The first time you open today's note it can already hold the day's meetings and the things
      you brought up for today, as tick boxes, with a Notes heading to write under. Nothing is
      kept until you type or tick something. Turn both off for a blank note.
    </p>
    <label class="check-row">
      <input
        type="checkbox"
        class="daily-meetings"
        checked={prefs.dailyMeetings}
        onchange={(e) => savePref('notes.daily_meetings', (e.currentTarget as HTMLInputElement).checked ? 'true' : 'false')}
      />
      Today's meetings, with their times
    </label>
    <label class="check-row">
      <input
        type="checkbox"
        class="daily-brought"
        checked={prefs.dailyBrought}
        onchange={(e) => savePref('notes.daily_brought', (e.currentTarget as HTMLInputElement).checked ? 'true' : 'false')}
      />
      What you brought up for today
    </label>
  </article>
{/if}
