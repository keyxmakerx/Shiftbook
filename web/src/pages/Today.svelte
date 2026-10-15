<script lang="ts">
  import { api } from '../lib/api';
  import { formatLongDate, toIsoDate } from '../lib/dates';

  const today = new Date();
  let version = $state<string | null>(null);

  $effect(() => {
    api
      .appInfo()
      .then((info) => {
        version = info.version;
      })
      .catch(() => {
        // Not running inside the desktop app (e.g. plain browser preview): no version to show.
        version = null;
      });
  });
</script>

<main>
  <header>
    <h1>Today</h1>
    <p class="date"><time datetime={toIsoDate(today)}>{formatLongDate(today)}</time></p>
  </header>

  <section class="empty" aria-label="Today's entries">
    <p>Nothing logged yet today</p>
  </section>
</main>

<footer>
  {#if version}
    <small>Shiftbook {version}</small>
  {/if}
</footer>

<style>
  main {
    max-width: 48rem;
    margin: 0 auto;
    padding: 2rem 1.5rem 1rem;
  }

  h1 {
    margin: 0;
    font-size: 2rem;
    font-weight: 600;
  }

  .date {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 1.125rem;
  }

  .empty {
    margin-top: 3rem;
    padding: 2rem 1rem;
    text-align: center;
    color: var(--muted);
    font-size: 1.125rem;
  }

  footer {
    max-width: 48rem;
    margin: 0 auto;
    padding: 1rem 1.5rem 2rem;
    color: var(--muted);
  }
</style>
