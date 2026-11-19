<script lang="ts">
  // A ticket's MOP (round 18): "MOP · Ready" opens it; with `offer`, a ticket without one gets
  // "+ MOP" to pick a template. In the tickets box, the slide-out and on the Tickets page; a
  // Desk card's front shows only a MOP the ticket has (round 19).
  import { attachMop, mopFor, openMopWindow, STATUS_WORD } from '../lib/mops.svelte';
  import { ownName } from '../lib/mop';

  let { ticket, title = '', offer = false }: { ticket: string; title?: string; offer?: boolean } = $props();
  const run = $derived(mopFor(ticket));
  // The name the owner gave it shows on the chip, so a ticket's card says which MOP it has.
  const name = $derived(run ? ownName(run.title, ticket) : '');
</script>

{#if run}
  <button
    type="button"
    class="chip {run.status}"
    title={name ? `Open “${name}”, the MOP for ${ticket}` : `Open the MOP for ${ticket}`}
    onpointerdown={(e) => e.stopPropagation()}
    onclick={(e) => {
      e.stopPropagation();
      openMopWindow(run.mopId);
    }}
  ><span class="st">MOP · {STATUS_WORD[run.status]}</span>{#if name}<span class="nm">· {name}</span>{/if}</button>
{:else if offer}
  <button
    type="button"
    class="chip add"
    title="Attach a MOP to {ticket} from a template"
    onpointerdown={(e) => e.stopPropagation()}
    onclick={(e) => {
      e.stopPropagation();
      attachMop(ticket, title);
    }}
  >+ MOP</button>
{/if}

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    border-radius: 7px;
    padding: 0 7px;
    font: 600 11px/18px var(--ui);
    cursor: pointer;
    white-space: nowrap;
    gap: 4px;
    max-width: 100%;
    min-width: 0;
    border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent);
    background: color-mix(in srgb, var(--ok) 10%, transparent);
    color: var(--ok);
  }

  .st {
    flex: none;
  }

  /* The MOP's name, cut short with … where the chip has no more room. */
  .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
    opacity: 0.85;
  }

  .chip.draft {
    border-color: var(--rule);
    background: transparent;
    color: var(--ink-2);
  }

  .chip.running {
    border-color: var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    color: var(--warn);
  }

  .chip.backed_out {
    color: var(--bad, #b03a2e);
    border-color: color-mix(in srgb, var(--bad, #b03a2e) 40%, transparent);
    background: transparent;
  }

  .chip.add {
    border-style: dashed;
    border-color: var(--rule);
    background: transparent;
    color: var(--mut);
  }

  .chip.add:hover {
    color: var(--ok);
    border-color: var(--ok);
  }
</style>
