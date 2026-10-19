<script lang="ts">
  // The paper context menu. The object lifts (a copy of it is drawn above everything), a
  // paper sheet pulls out from under its edge with the full options, and icon rails run
  // along its side and top. Click anywhere else or press Esc to fold it back under.
  import { tick } from 'svelte';
  import { menuState, registerCloser, type MenuItem } from '../lib/menu.svelte';
  import { placeMenu, type Placement } from '../lib/menuPlacement';
  import { scale } from '../lib/motion';
  import { SWATCHES } from '../lib/palette';
  import Icon from './Icon.svelte';

  let layer: HTMLDivElement | undefined = $state();
  let sheetEl: HTMLDivElement | undefined = $state();
  let sideEl: HTMLDivElement | undefined = $state();
  let topEl: HTMLDivElement | undefined = $state();
  let ghostEl: HTMLDivElement | undefined = $state();

  let pos = $state<Placement | null>(null);
  let rect = $state({ left: 0, top: 0, width: 0, height: 0 });
  let phase = $state<'idle' | 'measure' | 'open' | 'closing'>('idle');
  let ghostRadius = $state('8px');
  let ghostColor = $state('');

  let source: HTMLElement | null = null;
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  const spec = $derived(menuState.spec);
  const items = $derived(spec ? spec.sections.flatMap((s) => s.items ?? []) : []);
  const sideItems = $derived(items.filter((i) => i.side));
  const topItems = $derived(items.filter((i) => i.top));

  function teardown(returnFocus: boolean) {
    if (closeTimer !== null) clearTimeout(closeTimer);
    closeTimer = null;
    ghostEl?.replaceChildren();
    const was = source;
    was?.classList.remove('menu-source');
    source = null;
    phase = 'idle';
    pos = null;
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
    ghostRadius = cs.borderRadius && cs.borderRadius !== '0px' ? cs.borderRadius : '8px';
    ghostColor = cs.getPropertyValue('--c').trim();
    phase = 'measure';
    pos = null;
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
      copy.style.width = '100%';
      copy.style.height = '100%';
      copy.classList.remove('menu-source', 'rise', 'flash');
      ghostEl.replaceChildren(copy);
    }
    anchor.classList.add('menu-source');
    if (!sheetEl || !sideEl || !topEl) return;
    pos = placeMenu(
      rect,
      { width: window.innerWidth, height: window.innerHeight },
      { width: sheetEl.offsetWidth, height: sheetEl.offsetHeight },
      { width: sideEl.offsetWidth, height: sideEl.offsetHeight },
      { width: topEl.offsetWidth, height: topEl.offsetHeight },
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
    item.act();
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
    } else if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
      const rails = [...(topEl?.querySelectorAll<HTMLElement>('button') ?? [])];
      const i = rails.indexOf(document.activeElement as HTMLElement);
      if (i < 0) return;
      event.preventDefault();
      const step = event.key === 'ArrowRight' ? 1 : -1;
      rails[(i + step + rails.length) % rails.length]?.focus();
    }
  }

  function onScrimDown(event: PointerEvent) {
    event.preventDefault();
    close(false);
  }

  $effect(() => {
    if (phase !== 'open' && phase !== 'closing') return;
    const away = () => close(true);
    window.addEventListener('resize', away);
    window.addEventListener('wheel', away, { passive: true });
    window.addEventListener('blur', away);
    return () => {
      window.removeEventListener('resize', away);
      window.removeEventListener('wheel', away);
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

    <div
      class="rail side"
      class:left={pos?.side === 'left'}
      bind:this={sideEl}
      style:left={pos ? `${pos.sideX}px` : '-999px'}
      style:top={pos ? `${pos.sideY}px` : '0px'}
      role="toolbar"
      aria-orientation="vertical"
      aria-label="Quick actions"
    >
      {#each sideItems as item, i (item.label)}
        <button
          type="button"
          class="rbtn"
          class:danger={item.danger}
          style:--i={i}
          aria-label={item.label}
          data-tip={item.label}
          onclick={() => run(item)}
        >
          <Icon name={item.icon} size={17} />
        </button>
      {/each}
    </div>

    <div
      class="rail top"
      class:below={pos?.topWhere === 'below'}
      bind:this={topEl}
      style:left={pos ? `${pos.topX}px` : '-999px'}
      style:top={pos ? `${pos.topY}px` : '0px'}
      role="toolbar"
      aria-label="More quick actions"
    >
      {#each topItems as item, i (item.label)}
        <button
          type="button"
          class="rbtn"
          class:danger={item.danger}
          style:--i={i}
          aria-label={item.label}
          data-tip={item.label}
          onclick={() => run(item)}
        >
          <Icon name={item.icon} size={17} />
        </button>
      {/each}
    </div>
  </div>
{/if}

{#snippet sheetBody()}
  {#each spec?.sections ?? [] as section, si (si)}
    {#if si > 0}<div class="sep" role="separator"></div>{/if}
    {#if section.head}<div class="lbl head">{section.head}</div>{/if}
    {#each section.items ?? [] as item (item.label)}
      <button
        type="button"
        role="menuitem"
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
  {/each}
{/snippet}

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 60;
    pointer-events: none;
  }

  .layer > * {
    pointer-events: auto;
  }

  .measuring .clip,
  .measuring .rail {
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

  .rail {
    position: fixed;
    z-index: 3;
    display: flex;
    gap: 6px;
  }

  .rail.side {
    flex-direction: column;
  }

  .rail:empty {
    display: none;
  }

  .rbtn {
    position: relative;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--rule);
    background-color: var(--paper);
    background-image: var(--grain);
    color: var(--ink);
    box-shadow: var(--shadow);
    display: grid;
    place-items: center;
    cursor: pointer;
    animation: pop calc(0.38s * var(--m)) var(--bounce) both;
    animation-delay: calc((0.1s + var(--i) * 0.04s) * var(--m));
    transition:
      background-color 0.2s,
      transform 0.15s var(--bounce);
  }

  .side .rbtn {
    --from: translateX(-14px);
  }

  .side.left .rbtn {
    --from: translateX(14px);
  }

  .top .rbtn {
    --from: translateY(14px);
  }

  .top.below .rbtn {
    --from: translateY(-14px);
  }

  .rbtn:hover,
  .rbtn:focus-visible {
    background-color: var(--ink);
    color: var(--paper);
  }

  .rbtn.danger:hover,
  .rbtn.danger:focus-visible {
    background-color: var(--bad);
    color: #fff;
  }

  .closing .rbtn {
    animation: unpop calc(0.16s * var(--m)) var(--ease) forwards;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: var(--from) scale(0.5);
    }
  }

  @keyframes unpop {
    to {
      opacity: 0;
      transform: var(--from) scale(0.6);
    }
  }

  /* Tooltips for the icon buttons. */
  .rbtn::after {
    content: attr(data-tip);
    position: absolute;
    white-space: nowrap;
    font: 600 12px var(--ui);
    padding: 4px 8px;
    border-radius: 6px;
    background: var(--ink);
    color: var(--paper);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.15s;
  }

  .side .rbtn::after {
    left: calc(100% + 8px);
    top: 50%;
    transform: translateY(-50%);
  }

  .side.left .rbtn::after {
    left: auto;
    right: calc(100% + 8px);
  }

  .top .rbtn::after {
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
  }

  .top.below .rbtn::after {
    bottom: auto;
    top: calc(100% + 8px);
  }

  .rbtn:hover::after,
  .rbtn:focus-visible::after {
    opacity: 1;
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
