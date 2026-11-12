<script lang="ts">
  // A page's text, read (round 18): headings, numbered and bulleted lines, code blocks,
  // `code`, [[links]] you can click and {{blanks}} highlighted. Shared by the wiki and MOPs.
  import { blocks, type Part } from '../lib/wikiText';

  let { body, onlink }: { body: string; onlink: (title: string) => void } = $props();
  const read = $derived(blocks(body));
</script>

{#snippet line(ps: Part[])}
  {#each ps as p, i (i)}
    {#if p.kind === 'link'}
      <button type="button" class="wlink" onclick={() => onlink(p.title)}>{p.title}</button>
    {:else if p.kind === 'blank'}
      <span class="blank">{'{{'}{p.name}{'}}'}</span>
    {:else if p.kind === 'code'}
      <code>{p.text}</code>
    {:else}{p.text}{/if}
  {/each}
{/snippet}

<div class="read">
  {#each read as b, i (i)}
    {#if b.kind === 'heading'}
      {#if b.level === 1}<h2>{@render line(b.parts)}</h2>{:else if b.level === 2}<h3>{@render line(b.parts)}</h3>{:else}<h4>{@render line(b.parts)}</h4>{/if}
    {:else if b.kind === 'item'}
      <p class="item" style:padding-left="{18 + b.depth * 18}px"><span class="mk">{b.marker}</span>{@render line(b.parts)}</p>
    {:else if b.kind === 'code'}
      <pre>{b.text}</pre>
    {:else if b.kind === 'para'}
      <p class="para" style:padding-left={b.depth ? `${b.depth * 18}px` : undefined}>{@render line(b.parts)}</p>
    {:else}
      <div class="gap"></div>
    {/if}
  {:else}
    <p class="empty">Nothing written yet. Press Edit to start.</p>
  {/each}
</div>

<style>
  .read {
    font-size: 14.5px;
    line-height: 1.55;
    user-select: text;
  }

  .read h2,
  .read h3,
  .read h4 {
    font-family: var(--display);
    margin: 14px 0 4px;
  }

  .read h2 {
    font-size: 19px;
  }

  .read h3 {
    font-size: 16.5px;
  }

  .read h4 {
    font-size: 15px;
  }

  .read p {
    margin: 2px 0;
  }

  .item {
    position: relative;
  }

  /* Tabs and runs of spaces inside a line stay as typed (columns in a MOP line up). */
  .para {
    white-space: pre-wrap;
    tab-size: 4;
  }

  .mk {
    position: absolute;
    margin-left: -18px;
    color: var(--mut);
    min-width: 16px;
  }

  .gap {
    height: 8px;
  }

  pre,
  code {
    font-family: var(--mono);
    background: color-mix(in srgb, var(--ink) 6%, transparent);
    border-radius: 5px;
  }

  pre {
    padding: 8px 10px;
    margin: 6px 0;
    white-space: pre-wrap;
    font-size: 13px;
  }

  code {
    padding: 1px 5px;
    font-size: 13px;
  }

  .wlink {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    border-bottom: 1px dashed var(--accent);
    cursor: pointer;
  }

  .blank {
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    border: 1px dashed var(--warn);
    border-radius: 4px;
    padding: 0 4px;
    font: 600 13px var(--mono);
    color: var(--warn);
  }

  .empty {
    color: var(--mut);
    font-style: italic;
  }
</style>
