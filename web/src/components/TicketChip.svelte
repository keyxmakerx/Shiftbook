<script lang="ts">
  // A ticket chip: click to copy the number; right-click for its paper menu (copy, copy as
  // a link when a link field's prefix matches, log more time, remove).
  import type { TicketLink } from '../lib/api';
  import { paperMenu, type MenuItem, type MenuSpec } from '../lib/menu.svelte';
  import { copyText } from '../lib/protection.svelte';
  import { focusLog } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';

  let {
    number,
    links = [],
    color = 'var(--accent)',
    onRemove = null,
    removeLabel = 'Remove from this entry',
    showRemove = false,
  }: {
    number: string;
    links?: TicketLink[];
    color?: string;
    onRemove?: (() => void) | null;
    removeLabel?: string;
    /** Show a ✕ next to the chip for `onRemove` (it is always in the right-click menu too). */
    showRemove?: boolean;
  } = $props();

  async function copy(text: string, what: string) {
    toast((await copyText(text)) ? `Copied ${what}` : "Couldn't copy here.");
  }

  function menu(): MenuSpec {
    const items: MenuItem[] = [
      { label: 'Copy ticket number', icon: 'copy', side: true, act: () => void copy(number, number) },
      ...links.map((l) => ({
        label: `Copy ${l.fieldName} link`,
        icon: 'link' as const,
        side: true,
        act: () => void copy(l.url, `the ${l.fieldName} link`),
      })),
      { label: 'Log more time on it', icon: 'clock', top: true, act: () => focusLog(number) },
    ];
    const sections = [{ head: number, items }];
    if (onRemove) {
      const remove = onRemove;
      sections.push({
        head: '',
        items: [{ label: removeLabel, icon: 'trash', danger: true, top: true, act: remove }],
      });
    }
    return { sections };
  }
</script>

{#if showRemove && onRemove}<span class="pair">{@render chip()}<button
  type="button"
  class="off"
  title="{removeLabel} ({number})"
  aria-label="{removeLabel}: {number}"
  onclick={(e) => {
    e.stopPropagation();
    onRemove?.();
  }}>×</button></span>{:else}{@render chip()}{/if}

{#snippet chip()}
<button
  type="button"
  class="chip"
  style:--c={color}
  title="Click to copy · right-click for more"
  onclick={(e) => {
    e.stopPropagation();
    void copy(number, number);
  }}
  use:paperMenu={menu}
>
  {number}{#if links.length > 0}<span class="linked" aria-label="has a link">↗</span>{/if}
</button>
{/snippet}

<style>
  .pair {
    display: inline-flex;
    align-items: center;
    gap: 1px;
  }

  .off {
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--mut);
    font-size: 15px;
    line-height: 1;
    cursor: pointer;
  }

  .off:hover,
  .off:focus-visible {
    background: color-mix(in srgb, var(--bad) 14%, transparent);
    color: var(--bad);
  }

  .linked {
    font-size: 10px;
    opacity: 0.7;
  }
</style>
