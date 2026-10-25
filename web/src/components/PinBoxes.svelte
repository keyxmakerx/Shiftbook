<script lang="ts">
  // Six boxes for a 6-digit PIN (Quick unlock, round 15). One real input sits over the boxes,
  // so typing, pasting and screen readers work as in any text box; the boxes only show dots.
  // `onfull` runs once six digits are in.
  const DIGITS = 6;

  let {
    value = $bindable(''),
    label,
    disabled = false,
    autofocus = false,
    onfull,
  }: {
    value?: string;
    label: string;
    disabled?: boolean;
    autofocus?: boolean;
    onfull?: (pin: string) => void;
  } = $props();

  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (autofocus && !disabled) input?.focus();
  });

  function typed() {
    value = value.replace(/\D/g, '').slice(0, DIGITS);
    if (value.length === DIGITS) onfull?.(value);
  }

  /** Put focus back in the boxes (after a wrong PIN). */
  export function focus() {
    input?.focus();
  }
</script>

<div class="pin">
  <input
    bind:this={input}
    bind:value
    oninput={typed}
    inputmode="numeric"
    pattern="[0-9]*"
    maxlength={DIGITS}
    autocomplete="off"
    spellcheck="false"
    aria-label={label}
    {disabled}
  />
  <div class="boxes" aria-hidden="true">
    {#each Array(DIGITS) as _, i (i)}
      <span class="box" class:on={i === value.length}>{i < value.length ? '•' : ''}</span>
    {/each}
  </div>
</div>

<style>
  .pin {
    position: relative;
    display: inline-block;
  }

  input {
    position: absolute;
    inset: 0;
    width: 100%;
    opacity: 0;
    border: 0;
    font-size: 16px;
    cursor: text;
  }

  .boxes {
    display: flex;
    gap: 8px;
    pointer-events: none;
  }

  .box {
    width: 40px;
    height: 48px;
    border: 1px solid var(--rule);
    border-radius: 8px;
    background: var(--paper);
    display: grid;
    place-items: center;
    font: 20px var(--mono);
    color: var(--ink);
  }

  input:focus-visible + .boxes .box.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  input:disabled + .boxes {
    opacity: 0.6;
  }
</style>
