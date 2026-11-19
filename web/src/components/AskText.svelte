<script lang="ts">
  // A one-line question over everything (round 21), like DayPrompt but for words.
  import { answerText, textPrompt } from '../lib/askText.svelte';
  import { focusNow } from '../lib/focus';

  let text = $state('');

  let box: HTMLInputElement | undefined = $state();

  $effect(() => {
    void textPrompt.serial;
    text = textPrompt.value;
  });

  // The box can open a moment after a menu choice, while the menu is still folding away and
  // about to hand focus back to where it came from: take it once that has settled too.
  $effect(() => {
    if (!textPrompt.open || !box) return;
    const el = box;
    const t = setTimeout(() => {
      if (!el.isConnected || document.activeElement === el) return;
      el.focus();
      el.select();
    }, 320);
    return () => clearTimeout(t);
  });

  const valid = $derived(text.trim().length > 0);

  function ok() {
    if (valid) answerText(text.trim());
  }
</script>

{#if textPrompt.open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={() => answerText(null)}></div>
  <div class="box paper" role="dialog" aria-modal="true" aria-labelledby="text-prompt-title">
    <h2 id="text-prompt-title">{textPrompt.title}</h2>
    <input
      class="text-in"
      type="text"
      maxlength="80"
      bind:value={text}
      bind:this={box}
      use:focusNow
      aria-label={textPrompt.label}
      placeholder={textPrompt.label}
      onkeydown={(e) => {
        if (e.key === 'Enter') ok();
        else if (e.key === 'Escape') {
          e.stopPropagation();
          answerText(null);
        }
      }}
    />
    {#if textPrompt.hint}<p class="hint">{textPrompt.hint}</p>{/if}
    <div class="row">
      <button type="button" class="btn" onclick={() => answerText(null)}>Cancel</button>
      <button type="button" class="btn primary" disabled={!valid} onclick={ok}>{textPrompt.ok}</button>
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: color-mix(in srgb, var(--ink) 18%, transparent);
  }

  .box {
    position: fixed;
    z-index: 81;
    left: 50%;
    top: 30%;
    transform: translateX(-50%);
    width: min(340px, calc(100% - 32px));
    padding: 16px 18px;
    border-radius: 12px;
    box-shadow: var(--lift);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font: 700 17px var(--display);
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--mut);
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
