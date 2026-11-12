<script lang="ts">
  // The paper context menu. The object lifts (a copy of it is drawn above everything) and a
  // paper sheet pulls out from under its edge with the options. A row with a ▾ opens in place
  // to show its own choices (days, projects, colours), so the sheet stays short (the daily-use
  // review, 10 Nov: no ring of round icons, about a dozen rows). Click anywhere else or press
  // Esc to fold it back under.
  import { tick } from 'svelte';
  import { menuState, registerCloser, type MenuItem, type MenuSection } from '../lib/menu.svelte';
  import { growMenu, placeMenu, TUCK, type Placement } from '../lib/menuPlacement';
  import { scale } from '../lib/motion';
  import { SWATCHES } from '../lib/palette';
  import Icon from './Icon.svelte';

  let layer: HTMLDivElement | undefined = $state();
  let sheetEl: HTMLDivElement | undefined = $state();
  let ghostEl: HTMLDivElement | undefined = $state();
  /** The row whose own choices are open ("<section>:<label>"), if any. */
  let openSub = $state<string | null>(null);

  let pos = $state<Placement | null>(null);
  /** Where the sheet came out, before any row opened in place, and the height it asked for. */
  let first: Placement | null = null;
  let asked = 0;
  let rect = $state({ left: 0, top: 0, width: 0, height: 0 });
  let phase = $state<'idle' | 'measure' | 'open' | 'closing'>('idle');
  let ghostRadius = $state('8px');
  let ghostColor = $state('');

  let source: HTMLElement | null = null;
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  const spec = $derived(menuState.spec);
  /** The sheet grew over the object (a row opened with no room left): it goes on top of the
   *  lifted copy instead of under it, so none of its rows hide behind the object. */
  const over = $derived(
    !!pos && pos !== first && pos.sheetY < rect.top + rect.height - TUCK && pos.sheetY + pos.sheetHeight > rect.top + TUCK,
  );

  function teardown(returnFocus: boolean) {
    if (closeTimer !== null) clearTimeout(closeTimer);
    closeTimer = null;
    ghostEl?.replaceChildren();
    const was = source;
    was?.classList.remove('menu-source');
    source = null;
    phase = 'idle';
    pos = first = null;
    menuState.spec = null;
    menuState.anchor = null;
    if (returnFocus && was?.isConnected) was.focus({ preventScroll: true });
  }

  function close(immediate: boolean) {
    if (phase === 'idle') return;
    if (immediate || scale() === 0) {
      teardown(!immediate);
      return;
    }
    if (phase === 'closing') return;
    phase = 'closing';
    closeTimer = setTimeout(() => teardown(true), 230 * scale());
  }

  $effect(() => {
    registerCloser(close);
    return () => registerCloser(null);
  });

  /** Text settings an object inherits from where it sits, copied onto its lifted copy. */
  const INHERITED = [
    'color',
    'font-family',
    'font-size',
    'font-style',
    'font-weight',
    'letter-spacing',
    'line-height',
    'text-align',
    'text-transform',
    'white-space',
    'word-break',
    'overflow-wrap',
  ];

  /** The object's own box as drawn, copied too: a toolbar button takes its layout, padding and
   *  border from rules about the toolbar, which its copy no longer sits in. */
  const OWN_BOX = [
    'display',
    'flex-direction',
    'flex-wrap',
    'grid-auto-flow',
    'align-items',
    'justify-content',
    'justify-items',
    'row-gap',
    'column-gap',
    'padding-top',
    'padding-right',
    'padding-bottom',
    'padding-left',
    'background-color',
    ...['top', 'right', 'bottom', 'left'].flatMap((side) => ['width', 'style', 'color'].map((part) => `border-${side}-${part}`)),
  ];

  /** An element's own laid-out size from its computed style (exact), else the rounded one. */
  function ownSize(computed: string, rounded: number): number {
    const px = computed.endsWith('px') ? parseFloat(computed) : NaN;
    return Number.isFinite(px) && px > 0 ? px : rounded;
  }

  async function setup(anchor: HTMLElement) {
    if (closeTimer !== null) {
      clearTimeout(closeTimer);
      closeTimer = null;
    }
    source?.classList.remove('menu-source');
    source = anchor;
    const r = anchor.getBoundingClientRect();
    rect = { left: r.left, top: r.top, width: r.width, height: r.height };
    const cs = getComputedStyle(anchor);
    // The ghost's own corners match the object as drawn, zoom included.
    const zoom = anchor.offsetWidth > 0 ? r.width / anchor.offsetWidth : 1;
    ghostRadius =
      cs.borderRadius && cs.borderRadius !== '0px'
        ? cs.borderRadius.replace(/[\d.]+px/g, (px) => `${parseFloat(px) * zoom}px`)
        : '8px';
    ghostColor = cs.getPropertyValue('--c').trim();
    phase = 'measure';
    pos = first = null;
    openSub = null;
    await tick();
    // The lifted copy of the object.
    if (ghostEl) {
      const copy = anchor.cloneNode(true) as HTMLElement;
      copy.removeAttribute('id');
      copy.querySelectorAll('[id]').forEach((el) => el.removeAttribute('id'));
      copy.style.cssText = anchor.style.cssText;
      if (ghostColor) copy.style.setProperty('--c', ghostColor);
      // The copy sits in the ghost's box, not where the original was placed (Desk cards are
      // positioned on the board).
      copy.style.position = 'static';
      for (const side of ['left', 'top', 'right', 'bottom', 'z-index', 'transform']) copy.style.removeProperty(side);
      copy.style.margin = '0';
      // The copy no longer sits inside the object's parents, so it takes the text settings it
      // had from them along, and its own box as drawn. Without these a row could pick up
      // another font size or wrapping and break a line the object itself doesn't, and a
      // toolbar button would lose its layout.
      for (const prop of [...INHERITED, ...OWN_BOX]) copy.style.setProperty(prop, cs.getPropertyValue(prop));
      // On a zoomed Desk the object is drawn scaled, so its box on screen differs from its own
      // size. Lay the copy out at its own size and scale it to the box, so it looks the same
      // instead of squashing (or stretching) its contents. The size is the exact one, not
      // rounded to whole pixels: half a pixel narrower is enough to wrap a title.
      const w = ownSize(cs.width, anchor.offsetWidth);
      const h = ownSize(cs.height, anchor.offsetHeight);
      copy.style.width = `${w}px`;
      copy.style.height = `${h}px`;
      if (w > 0 && h > 0) {
        copy.style.transformOrigin = '0 0';
        copy.style.transform = `scale(${r.width / w}, ${r.height / h})`;
      }
      copy.classList.remove('menu-source', 'rise', 'flash');
      ghostEl.replaceChildren(copy);
    }
    anchor.classList.add('menu-source');
    if (!sheetEl) return;
    asked = sheetEl.offsetHeight;
    pos = first = placeMenu(
      rect,
      { width: window.innerWidth, height: window.innerHeight },
      { width: sheetEl.offsetWidth, height: asked },
    );
    phase = 'open';
    await tick();
    sheetEl.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true });
  }

  $effect(() => {
    void menuState.serial;
    const anchor = menuState.anchor;
    if (menuState.spec && anchor) void setup(anchor);
  });

  function run(item: MenuItem) {
    close(false);
    item.act?.();
  }

  /** Open a row's own choices in place (or fold them again): the sheet grows to show them,
   *  keeping its rows where they are as far as the window allows. */
  async function toggleSub(key: string) {
    openSub = openSub === key ? null : key;
    await tick();
    if (!sheetEl || !first) return;
    pos = growMenu(first, { width: window.innerWidth, height: window.innerHeight }, sheetEl.scrollHeight, asked);
    await tick();
    sheetEl.querySelector('.sub')?.scrollIntoView({ block: 'nearest' });
  }

  function focusables(): HTMLElement[] {
    return layer ? Array.from(layer.querySelectorAll<HTMLElement>('button:not([disabled])')) : [];
  }

  function onKeydown(event: KeyboardEvent) {
    if (phase !== 'open') return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close(false);
      return;
    }
    const list = focusables();
    const at = list.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Tab') {
      event.preventDefault();
      const next = (at + (event.shiftKey ? -1 : 1) + list.length) % list.length;
      list[next]?.focus();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      const inSheet = sheetEl ? Array.from(sheetEl.querySelectorAll<HTMLElement>('button')) : [];
      const i = inSheet.indexOf(document.activeElement as HTMLElement);
      if (inSheet.length === 0) return;
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      inSheet[(i + step + inSheet.length) % inSheet.length]?.focus();
    }
  }

  function onScrimDown(event: PointerEvent) {
    event.preventDefault();
    close(false);
  }

  $effect(() => {
    if (phase !== 'open' && phase !== 'closing') return;
    const away = () => close(true);
    // The wheel scrolls a long menu while the pointer is over it; anywhere else it closes it.
    const wheel = (e: WheelEvent) => {
      const t = e.target instanceof Element ? e.target : null;
      if (t && layer?.contains(t) && !t.closest('.scrim')) return;
      close(true);
    };
    window.addEventListener('resize', away);
    window.addEventListener('wheel', wheel, { passive: true });
    window.addEventListener('blur', away);
    return () => {
      window.removeEventListener('resize', away);
      window.removeEventListener('wheel', wheel);
      window.removeEventListener('blur', away);
    };
  });
</script>

{#if spec}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="layer"
    class:measuring={phase === 'measure'}
    class:closing={phase === 'closing'}
    bind:this={layer}
    onkeydown={onKeydown}
  >
    <div
      class="scrim"
      onpointerdown={onScrimDown}
      oncontextmenu={(e) => {
        e.preventDefault();
        close(false);
      }}
    ></div>

    {#if pos}
      <div
        class="clip"
        class:from-above={pos.sheetFrom === 'above'}
        class:over
        style:left="{pos.sheetX}px"
        style:top="{pos.sheetY}px"
      >
        <div class="paper sheet" role="menu" tabindex="-1" bind:this={sheetEl} style:max-height="{pos.sheetHeight}px">
          {@render sheetBody()}
        </div>
      </div>
    {:else}
      <div class="clip probe">
        <div class="paper sheet" role="menu" tabindex="-1" bind:this={sheetEl}>
          {@render sheetBody()}
        </div>
      </div>
    {/if}

    <div
      class="ghost"
      bind:this={ghostEl}
      style:left="{rect.left}px"
      style:top="{rect.top}px"
      style:width="{rect.width}px"
      style:height="{rect.height}px"
      style:border-radius={ghostRadius}
      aria-hidden="true"
    ></div>

  </div>
{/if}

{#snippet sheetBody()}
  {#each spec?.sections ?? [] as section, si (si)}
    {#if si > 0}<div class="sep" role="separator"></div>{/if}
    {@render sectionBody(section, `${si}`)}
  {/each}
{/snippet}

{#snippet sectionBody(section: MenuSection, at: string)}
  {#if section.head}<div class="lbl head">{section.head}</div>{/if}
  {#each section.items ?? [] as item (item.label)}
    {#if item.sub}
      {@const key = `${at}:${item.label}`}
      <button
        type="button"
        role="menuitem"
        class="item"
        aria-haspopup="true"
        aria-expanded={openSub === key}
        onclick={() => void toggleSub(key)}
      >
        <Icon name={item.icon} size={15} />
        <span class="label">{item.label}</span>
        {#if item.value}<span class="value">{item.value}</span>{/if}
        <Icon name={openSub === key ? 'up' : 'down'} size={13} />
      </button>
      {#if openSub === key}
        <div class="sub" role="group" aria-label={item.label}>
          {#each item.sub as part, pi (pi)}
            {@render sectionBody(part, `${key}/${pi}`)}
          {/each}
        </div>
      {/if}
    {:else}
      <button
        type="button"
        role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
        aria-checked={item.checked}
        class="item"
        class:danger={item.danger}
        onclick={() => run(item)}
      >
        {#if item.dot !== undefined}
          <i class="dot" style:--c={item.dot ?? 'var(--none)'}></i>
        {:else}
          <Icon name={item.icon} size={15} />
        {/if}
        <span class="label">{item.label}</span>
        {#if item.checked}<Icon name="check" size={14} />{/if}
        {#if item.hint}<span class="kbd">{item.hint}</span>{/if}
      </button>
    {/if}
  {/each}
  {#if section.choices}
    <div class="choices" role="group" aria-label={section.head ?? 'Choices'}>
      {#each section.choices as choice (choice.label)}
        <button
          type="button"
          class="choice"
          style:--c={choice.color ?? 'var(--none)'}
          aria-pressed={choice.checked}
          onclick={() => {
            close(false);
            choice.act();
          }}
        >
          {choice.label}
        </button>
      {/each}
    </div>
  {/if}
  {#if section.swatches}
    {@const sw = section.swatches}
    <div class="swatches" role="group" aria-label="Colours">
      {#each SWATCHES as color (color)}
        <button
          type="button"
          class="sw"
          style:--c={color}
          aria-label="Colour {color}"
          aria-pressed={sw.current?.toLowerCase() === color}
          onclick={() => {
            close(false);
            sw.pick(color);
          }}
        ></button>
      {/each}
    </div>
  {/if}
{/snippet}

<style>
  /* Above the toast (70), so a message that's showing can't cover the menu's options. */
  .layer {
    position: fixed;
    inset: 0;
    z-index: 75;
    pointer-events: none;
  }

  .layer > * {
    pointer-events: auto;
  }

  .measuring .clip {
    visibility: hidden;
  }

  .scrim {
    position: fixed;
    inset: 0;
  }

  /* The sheet lives in a clip box that starts at the object's edge, so it seems to slide
     out from under the object. */
  .clip {
    position: fixed;
    z-index: 1;
    overflow: hidden;
    padding: 0 14px 22px;
    margin: 0 -14px;
    perspective: 900px;
  }

  .clip.from-above {
    padding: 22px 14px 0;
    margin-top: -22px;
  }

  .clip.over {
    z-index: 3;
  }

  .clip.probe {
    left: -9999px;
    top: 0;
  }

  .sheet {
    width: 252px;
    padding: 5px;
    overflow-y: auto;
    overscroll-behavior: contain;
    box-shadow: var(--lift);
    transform-origin: top center;
    animation: pull-out calc(0.5s * var(--m)) var(--spring) both;
    animation-delay: calc(0.05s * var(--m));
    outline: 0;
  }

  .from-above .sheet {
    transform-origin: bottom center;
    animation-name: pull-out-up;
  }

  .closing .sheet {
    animation: tuck-in calc(0.22s * var(--m)) var(--ease) forwards;
  }

  .closing .from-above .sheet {
    animation-name: tuck-in-up;
  }

  @keyframes pull-out {
    from {
      transform: translateY(-100%) rotateX(-24deg) scaleY(0.92);
    }
    to {
      transform: none;
    }
  }

  @keyframes pull-out-up {
    from {
      transform: translateY(100%) rotateX(24deg) scaleY(0.92);
    }
    to {
      transform: none;
    }
  }

  @keyframes tuck-in {
    to {
      transform: translateY(-100%) rotateX(-18deg);
    }
  }

  @keyframes tuck-in-up {
    to {
      transform: translateY(100%) rotateX(18deg);
    }
  }

  /* The lifted object. */
  .ghost {
    position: fixed;
    z-index: 2;
    background-color: var(--paper);
    background-image: var(--grain);
    pointer-events: none;
    animation: lift calc(0.36s * var(--m)) var(--spring) both;
    box-shadow: var(--lift);
    transform: translateY(-3px) scale(1.012);
  }

  .closing .ghost {
    animation: settle calc(0.22s * var(--m)) var(--ease) both;
  }

  @keyframes lift {
    from {
      transform: none;
      box-shadow: none;
    }
  }

  @keyframes settle {
    to {
      transform: none;
      box-shadow: none;
    }
  }

  .ghost :global(*) {
    pointer-events: none;
  }

  .head {
    padding: 7px 10px 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--rule);
  }

  .item {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 9px;
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13.5px;
    color: var(--ink);
    min-height: 31px;
  }

  .item .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item:hover,
  .item:focus-visible {
    background: color-mix(in srgb, var(--ink) 7%, transparent);
    outline: 0;
  }

  .item.danger {
    color: var(--bad);
  }

  /* What a row with its own choices is set to now (a project, a day). */
  .item .value {
    max-width: 96px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--muted);
  }

  .item[aria-expanded='true'] {
    background: color-mix(in srgb, var(--ink) 5%, transparent);
  }

  /* A row's own choices, opened in place under it. */
  .sub {
    margin: 0 0 4px 18px;
    padding-left: 6px;
    border-left: 2px solid var(--rule);
  }

  .sub .head {
    padding-left: 4px;
  }

  /* An opened row is narrower than the sheet: its colours go in two rows of five. */
  .sub .swatches {
    grid-template-columns: repeat(5, max-content);
    justify-content: start;
    gap: 6px 8px;
    padding: 4px 4px 8px;
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 3px;
    padding: 2px 8px 8px;
    justify-items: center;
  }

  .swatches .sw {
    width: 19px;
    height: 19px;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 2px 8px 8px;
  }

  .choice {
    border: 1.5px solid color-mix(in srgb, var(--c) 45%, transparent);
    background: none;
    border-radius: 99px;
    padding: 2px 9px;
    cursor: pointer;
    font-size: 12px;
    color: var(--ink);
    min-height: 26px;
  }

  .choice:hover,
  .choice:focus-visible {
    background: color-mix(in srgb, var(--c) 14%, transparent);
    outline: 0;
  }

  .choice[aria-pressed='true'] {
    background: var(--c);
    border-color: var(--c);
    color: #fff;
  }
</style>
