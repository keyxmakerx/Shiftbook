<script lang="ts">
  // "Copy MOP files in" (round 16): files dropped anywhere on Shiftbook, or picked with Add
  // MOP files…, are listed here first. Each comes in as an encrypted MOP or a template; the
  // files themselves are never changed. Dragging files over the app shows where to drop.
  import { api } from '../lib/api';
  import { hasFiles, looksLikeText } from '../lib/mop';
  import { mopImport, offerFiles, openMopBook } from '../lib/mopbook.svelte';
  import { attempt, words } from '../lib/store.svelte';
  import { toast } from '../lib/toast.svelte';
  import { trapTab } from '../lib/trap';
  import { focusNow } from '../lib/focus';
  import Icon from './Icon.svelte';

  type Row = { name: string; text: string; take: boolean; as: 'mop' | 'template' };

  let rows = $state<Row[]>([]);
  let skipped = $state<{ name: string; why: string }[]>([]);
  let busy = $state(false);
  let over = $state(false);

  // Fresh rows each time files arrive.
  $effect(() => {
    const files = mopImport.files;
    if (!files) return;
    rows = files.map((f) => ({ name: f.name, text: f.text, take: looksLikeText(f.name), as: f.template ? 'template' : 'mop' }));
    skipped = [];
  });

  const taking = $derived(rows.filter((r) => r.take).length);

  function close() {
    mopImport.files = null;
    rows = [];
    skipped = [];
  }

  async function bringIn() {
    busy = true;
    const files = rows.filter((r) => r.take).map((r) => ({ name: r.name, text: r.text, template: r.as === 'template' }));
    const done = await attempt(() => api.importMops(files));
    busy = false;
    if (!done) return;
    for (const n of done.made) openMopBook(n.id);
    if (done.made.length) {
      const n = done.made.length;
      toast(`${n} ${words().mop}${n === 1 ? '' : 's'} copied in and encrypted. Your files are unchanged.`);
    }
    if (done.skipped.length) {
      skipped = done.skipped;
      rows = [];
    } else {
      close();
    }
  }
</script>

<svelte:window
  ondragover={(e) => {
    if (!hasFiles(e)) return;
    // Files dropped outside a drop spot would otherwise open in the window.
    e.preventDefault();
    over = true;
  }}
  ondragleave={(e) => {
    if (!e.relatedTarget) over = false;
  }}
  ondrop={(e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    over = false;
    if (e.dataTransfer?.files.length) void offerFiles(e.dataTransfer.files);
  }}
  ondragend={() => (over = false)}
/>

{#if over}
  <div class="hint" aria-hidden="true">
    <Icon name="upload" size={20} />
    <b>Drop {words().mop} files to copy them in</b>
    <span>Your files stay where they are.</span>
  </div>
{/if}

{#if mopImport.files}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="scrim"
    role="dialog"
    aria-modal="true"
    aria-label="Copy {words().mop} files in"
    tabindex="-1"
    onkeydown={(e) => {
      trapTab(e, e.currentTarget as HTMLElement);
      if (e.key === 'Escape') close();
    }}
  >
    <div class="sheet paper">
      <h3>Copy {words().mop} files in</h3>
      {#if skipped.length}
        <p>{skipped.length === 1 ? "This file couldn't come in:" : "These files couldn't come in:"}</p>
        <ul class="list">
          {#each skipped as s (s.name)}
            <li><b>{s.name}</b><span class="why">{s.why}</span></li>
          {/each}
        </ul>
        <div class="row">
          <button type="button" class="btn primary" use:focusNow onclick={close}>OK</button>
        </div>
      {:else}
        <p class="mut">Each file becomes an encrypted {words().mop} in Shiftbook. A template keeps its [[blanks]] to fill in each time. Your files stay where they are, unchanged.</p>
        <ul class="list">
          {#each rows as r, i (i)}
            <li>
              <label class="take">
                <input type="checkbox" bind:checked={r.take} />
                <span class="name">{r.name}</span>
              </label>
              <div class="seg" role="group" aria-label="Bring {r.name} in as">
                <button type="button" aria-pressed={r.as === 'mop'} disabled={!r.take} onclick={() => (r.as = 'mop')}>{words().mop}</button>
                <button type="button" aria-pressed={r.as === 'template'} disabled={!r.take} onclick={() => (r.as = 'template')}>Template</button>
              </div>
            </li>
          {/each}
        </ul>
        <p class="tip">Word files (.docx) can't be read yet: in Word, use Save As, Plain Text, then drop the .txt in.</p>
        <div class="row">
          <button type="button" class="btn" onclick={close}>Cancel</button>
          <button type="button" class="btn primary" disabled={!taking || busy} use:focusNow onclick={() => void bringIn()}>
            {busy ? 'Copying…' : `Copy ${taking} in`}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .hint {
    position: fixed;
    inset: 12px;
    border: 2px dashed var(--accent);
    border-radius: 16px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    color: var(--accent);
    pointer-events: none;
    z-index: 30;
  }

  .hint span {
    color: var(--ink-2);
    font-size: 13px;
  }

  .scrim {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 16px;
    background: color-mix(in srgb, var(--ink) 30%, transparent);
    pointer-events: auto;
    z-index: 60;
  }

  .sheet {
    width: min(520px, 100%);
    max-height: 100%;
    overflow: auto;
    border-radius: 14px;
    padding: 18px 20px;
    box-shadow: var(--shadow);
    display: grid;
    gap: 12px;
  }

  h3 {
    margin: 0;
    font: 650 17px var(--display);
  }

  p {
    margin: 0;
  }

  .mut,
  .tip {
    color: var(--mut);
    font-size: 13px;
  }

  .tip {
    font-size: 12px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
    max-height: 300px;
    overflow: auto;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border: 1px solid var(--rule);
    border-radius: 8px;
  }

  .take {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13.5px;
  }

  .why {
    display: block;
    color: var(--mut);
    font-size: 12.5px;
  }

  .seg {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    overflow: hidden;
    flex: none;
  }

  .seg button {
    border: 0;
    background: none;
    padding: 4px 10px;
    font: 600 12px var(--ui);
    color: var(--ink-2);
    cursor: pointer;
  }

  .seg button + button {
    border-left: 1px solid var(--line-strong);
  }

  .seg button[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 14%, var(--paper));
    color: var(--ink);
  }

  .seg button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .row {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
</style>
