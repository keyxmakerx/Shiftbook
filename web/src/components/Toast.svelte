<script lang="ts">
  import { holdToast, releaseToast, runToastAction, toastState } from '../lib/toast.svelte';
</script>

<div
  class="toast"
  class:on={toastState.on}
  role="status"
  aria-live="polite"
  onpointerenter={holdToast}
  onpointerleave={releaseToast}
  onfocusin={holdToast}
  onfocusout={releaseToast}
>
  <span>{toastState.text}</span>
  {#if toastState.action}
    <!-- Out of the tab order while hidden, so a faded-out Undo can't be reached. -->
    <button type="button" tabindex={toastState.on ? 0 : -1} aria-hidden={!toastState.on} onclick={runToastAction}>{toastState.action}</button>
  {/if}
</div>

<style>
  .toast {
    position: fixed;
    z-index: 70;
    left: 50%;
    bottom: 18px;
    transform: translate(-50%, 30px) scale(0.95);
    opacity: 0;
    pointer-events: none;
    transition: calc(0.4s * var(--m)) var(--spring);
    background: var(--ink);
    color: var(--paper);
    padding: 9px 12px 9px 16px;
    border-radius: 12px;
    display: flex;
    gap: 12px;
    align-items: center;
    box-shadow: 0 12px 30px -10px rgba(0, 0, 0, 0.45);
    max-width: calc(100% - 32px);
    font-size: 13.5px;
  }

  .toast.on {
    transform: translate(-50%, 0);
    opacity: 1;
    pointer-events: auto;
  }

  button {
    border: 0;
    background: color-mix(in srgb, var(--paper) 18%, transparent);
    color: var(--paper);
    border-radius: 7px;
    padding: 4px 10px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }
</style>
