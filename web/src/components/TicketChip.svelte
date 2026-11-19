<script lang="ts">
  // A ticket chip: click to copy the number; right-click for its paper menu (copy, open or
  // copy its link when a link field's prefix matches, log more time, remove). A ticket with
  // a link gets a small ↗ beside it that opens the link in the browser (round 21).
  import type { TicketLink } from '../lib/api';
  import { paperMenu, type MenuItem, type MenuSpec } from '../lib/menu.svelte';
  import { copyText } from '../lib/protection.svelte';
  import { focusLog } from '../lib/store.svelte';
  import { openLinkItems, openLinks } from '../lib/ticketLinks';
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
      { label: 'Copy ticket number', icon: 'copy', act: () => void copy(number, number) },
      ...openLinkItems(number, links),
      ...links.map((l) => ({
        label: `Copy ${l.fieldName} link`,
        icon: 'link' as const,
        act: () => void copy(l.url, `the ${l.fieldName} link`),
      })),
      { label: 'Log more time on it', icon: 'clock', act: () => focusLog(number) },
    ];
    const sections = [{ head: number, items }];
    if (onRemove) {
      const remove = onRemove;
      sections.push({
        head: '',
        items: [{ label: removeLabel, icon: 'trash', danger: true, act: remove }],
      });
    }
    return { sections };
  }
</script>

{#if links.length > 0 && !(showRemove && onRemove)}<span class="pair">{@render chip()}{@render go()}</span>{:else if showRemove && onRemove}<span class="pair">{@render chip()}{@render go()}<button
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
  {number}
</button>
{/snippet}

{#snippet go()}
{#if links.length > 0}<button
  type="button"
  class="off go"
  title={links.length === 1 ? `Open the ${links[0]!.fieldName} link in your browser` : 'Open one of its links in your browser'}
  aria-label="Open {number} in your browser"
  onclick={(e) => {
    e.stopPropagation();
    openLinks(e.currentTarget, number, links);
  }}>↗</button>{/if}
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

  .off.go {
    font-size: 12px;
  }

  .off.go:hover,
  .off.go:focus-visible {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }
</style>
