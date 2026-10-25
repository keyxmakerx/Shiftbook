<script lang="ts">
  // Hold Ctrl (⌘ on a Mac) on its own for a moment to see every shortcut; let go to hide it.
  import { HOLD_MS, shortcutList, startsHold } from '../lib/keys';
  import { shownViews } from '../lib/store.svelte';

  const list = $derived(shortcutList(shownViews()));

  let shown = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function cancel() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    shown = false;
  }

  function down(event: KeyboardEvent) {
    if (event.repeat) return;
    if (startsHold(event)) {
      cancel();
      timer = setTimeout(() => (shown = true), HOLD_MS);
    } else {
      // Any shortcut pressed with Ctrl means it isn't a hold.
      cancel();
    }
  }

  function up(event: KeyboardEvent) {
    if (event.key === 'Control' || event.key === 'Meta') cancel();
  }
</script>

<svelte:window onkeydown={down} onkeyup={up} onblur={cancel} onpointerdown={cancel} />

<div class="paper keys" class:on={shown} aria-hidden={!shown}>
  <h2>Shortcuts</h2>
  <dl>
    {#each list as s, i (i)}
      <dt><span class="kbd">{s.keys}</span></dt>
      <dd>{s.what}</dd>
    {/each}
  </dl>
  <p class="foot">Let go of Ctrl to close.</p>
</div>

<style>
  .keys {
    position: fixed;
    z-index: 80;
    left: 50%;
    top: 50%;
    width: min(440px, calc(100% - 32px));
    padding: 18px 20px 14px;
    box-shadow: var(--lift);
    transform: translate(-50%, -46%) scale(0.97);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity calc(0.18s * var(--m)),
      transform calc(0.3s * var(--m)) var(--spring);
  }

  .keys.on {
    opacity: 1;
    transform: translate(-50%, -50%);
  }

  h2 {
    margin: 0 0 10px;
    font: 650 20px var(--display);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 7px 14px;
    margin: 0;
    align-items: center;
  }

  dt {
    justify-self: start;
  }

  dt .kbd {
    font-size: 12px;
    padding: 1px 6px;
    white-space: nowrap;
  }

  dd {
    margin: 0;
    font-size: 13.5px;
  }

  .foot {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--mut);
  }
</style>
