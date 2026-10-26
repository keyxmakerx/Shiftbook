<script lang="ts">
  // Quick unlock (round 15): after the day's password unlock, ask once for today's PIN so
  // locks until midnight can be opened with it. The PIN is never stored; Skip means locks
  // ask for the password today.
  import { api } from '../lib/api';
  import { toIsoDate } from '../lib/dates';
  import { errorMessage } from '../lib/messages';
  import { toast } from '../lib/toast.svelte';
  import { trapTab } from '../lib/trap';
  import PinBoxes from './PinBoxes.svelte';

  let { onDone }: { onDone: () => void } = $props();

  let pin = $state('');
  let working = $state(false);
  let error = $state('');
  let box: HTMLElement | undefined = $state();

  async function use(typed: string) {
    if (working) return;
    working = true;
    error = '';
    try {
      await api.startQuickUnlock(typed, toIsoDate(new Date()));
      toast('Quick unlock is ready. Until midnight, locks open with your PIN.');
      onDone();
    } catch (e) {
      pin = '';
      error = errorMessage(e);
    } finally {
      working = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    trapTab(e, box);
    if (e.key === 'Escape') onDone();
  }}
/>
<div class="scrim" aria-hidden="true"></div>
<div bind:this={box} class="paper prompt" role="dialog" aria-modal="true" aria-labelledby="today-pin-title">
  <h2 id="today-pin-title">Quick unlock today</h2>
  <p class="sub">Type your PIN once, and locks until midnight open with it. Three wrong PINs and it's your password again.</p>
  <PinBoxes bind:value={pin} label="Your PIN" disabled={working} autofocus onfull={(p) => void use(p)} />
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="row">
    <button type="button" class="btn" onclick={onDone} disabled={working}>Skip today</button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: rgba(0, 0, 0, 0.3);
  }

  .prompt {
    position: fixed;
    z-index: 81;
    left: 50%;
    top: 30%;
    transform: translateX(-50%);
    width: min(400px, calc(100% - 32px));
    padding: 22px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  h2 {
    margin: 0;
    font: 700 20px var(--display);
  }

  .sub {
    margin: 0;
    color: var(--ink-2);
    font-size: 13.5px;
  }

  .row {
    display: flex;
    gap: 8px;
  }
</style>
