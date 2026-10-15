<script lang="ts">
  import { passwordStrength } from '../lib/password';

  let { password, id }: { password: string; id: string } = $props();

  const strength = $derived(passwordStrength(password));
</script>

<div class="meter" {id} aria-live="polite">
  <div class="bars" aria-hidden="true">
    {#each [1, 2, 3, 4] as step (step)}
      <span class="bar" class:on={strength.level >= step} data-level={strength.level}></span>
    {/each}
  </div>
  <p class="label">
    Strength: <strong>{password.length === 0 ? '—' : strength.label}</strong>
    {#if password.length > 0 && strength.hint}
      <span class="hint">{strength.hint}</span>
    {/if}
  </p>
</div>

<style>
  .meter {
    margin-top: 0.5rem;
  }

  .bars {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.25rem;
  }

  .bar {
    height: 0.5rem;
    border-radius: 0.25rem;
    background: var(--line);
  }

  .bar.on[data-level='1'] {
    background: var(--danger);
  }

  .bar.on[data-level='2'] {
    background: var(--warn);
  }

  .bar.on[data-level='3'],
  .bar.on[data-level='4'] {
    background: var(--ok);
  }

  .label {
    margin: 0.25rem 0 0;
    color: var(--muted);
  }

  .hint {
    display: block;
  }
</style>
