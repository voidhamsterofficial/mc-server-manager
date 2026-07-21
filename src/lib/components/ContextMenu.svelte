<script lang="ts">
  // The custom right-click menu, styled like an in-game tooltip: dark
  // panel, chunky outline, pixel-font entries.

  import { fade } from "svelte/transition";
  import { contextMenuStore, type MenuItem } from "../stores/contextMenu.svelte";

  /** Index of the item whose submenu is open, or null. */
  let openSubmenuIndex = $state<number | null>(null);

  function run(item: MenuItem) {
    if (item.disabled) {
      return;
    }
    contextMenuStore.close();
    item.action();
  }

  function hasSubmenu(item: MenuItem): boolean {
    return item.submenu !== undefined;
  }

  function openSubmenu(index: number, item: MenuItem) {
    if (item.disabled || !hasSubmenu(item)) {
      openSubmenuIndex = null;
      return;
    }
    openSubmenuIndex = index;
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      contextMenuStore.close();
    }
  }
</script>

<svelte:window
  onclick={() => contextMenuStore.close()}
  onkeydown={handleWindowKeydown}
  onresize={() => contextMenuStore.close()}
  onblur={() => contextMenuStore.close()}
/>

{#if contextMenuStore.open}
  <div
    class="menu"
    style:left="{contextMenuStore.x}px"
    style:top={contextMenuStore.bottom === null ? `${contextMenuStore.y}px` : null}
    style:bottom={contextMenuStore.bottom === null ? null : `${contextMenuStore.bottom}px`}
    transition:fade={{ duration: 80 }}
    role="menu"
  >
    {#each contextMenuStore.entries as entry, index (index)}
      {#if entry === "separator"}
        <div class="separator"></div>
      {:else}
        <!-- The row owns the hover so the submenu stays open while the
             pointer travels across the gap into it. -->
        <div
          class="row"
          role="none"
          onmouseenter={() => openSubmenu(index, entry)}
          onmouseleave={() => (openSubmenuIndex = null)}
        >
          <button
            class="item"
            class:danger={entry.danger}
            disabled={entry.disabled}
            role="menuitem"
            aria-haspopup={hasSubmenu(entry) ? "menu" : undefined}
            aria-expanded={hasSubmenu(entry) ? openSubmenuIndex === index : undefined}
            onclick={() => run(entry)}
          >
            <span class="icon" class:danger={entry.danger} class:tone-success={entry.tone === "success"} class:tone-warning={entry.tone === "warning"} class:tone-info={entry.tone === "info"}>
              {#if entry.icon}
                <entry.icon size={15} />
              {/if}
            </span>
            {entry.label}
            {#if hasSubmenu(entry)}<span class="arrow">›</span>{/if}
          </button>

          {#if hasSubmenu(entry) && openSubmenuIndex === index}
            <div class="submenu" role="menu">
              {#if entry.submenu!.length === 0}
                <span class="submenu-empty">
                  {entry.emptySubmenuLabel ?? "Nothing to choose"}
                </span>
              {:else}
                {#each entry.submenu! as child, childIndex (childIndex)}
                  <button class="item" role="menuitem" onclick={() => run(child)}>
                    <span class="icon" class:tone-info={child.tone === "info"}>
                      {#if child.icon}
                        <child.icon size={15} />
                      {/if}
                    </span>
                    {child.label}
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 300;
    width: 230px;
    padding: 5px;
    background: #1d1e22;
    border-radius: var(--radius-sm);
    /* Tooltip-style double outline. */
    box-shadow:
      0 0 0 2px #101014,
      inset 0 0 0 1px rgba(255, 255, 255, 0.08),
      0 6px 0 rgba(0, 0, 0, 0.35);
  }

  .row {
    position: relative;
  }

  .arrow {
    margin-left: auto;
    padding-left: 0.4rem;
    opacity: 0.6;
  }

  /* Opens to the right, overlapping the parent's edge slightly so the pointer
     never crosses a dead gap on its way over. Flips to the left via `right`
     when the menu is close to the window edge. */
  .submenu {
    position: absolute;
    left: calc(100% - 2px);
    top: -5px;
    z-index: 1;
    width: 190px;
    max-height: 320px;
    overflow-y: auto;
    padding: 5px;
    background: #1d1e22;
    border-radius: var(--radius-sm);
    box-shadow:
      0 0 0 2px #101014,
      inset 0 0 0 1px rgba(255, 255, 255, 0.08),
      0 6px 0 rgba(0, 0, 0, 0.35);
  }

  .submenu-empty {
    display: block;
    padding: 0.45rem 0.6rem;
    font-size: 0.8rem;
    color: #8b8b95;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    border: none;
    background: transparent;
    color: #e6e6ea;
    font-family: var(--font-pixel);
    font-size: 0.82rem;
    text-align: left;
    padding: 0.5rem 0.6rem;
    border-radius: 3px;
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .item:hover:not(:disabled) {
    background: #2c3e26;
    color: #8ef05c;
  }

  .item.danger:hover:not(:disabled) {
    background: #40201f;
    color: #ff6b5e;
  }

  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon {
    display: inline-flex;
    width: 15px;
    flex-shrink: 0;
  }

  .icon.danger {
    color: var(--strawberry);
  }

  .icon.tone-success {
    color: var(--mint);
  }

  .icon.tone-warning {
    color: var(--peach);
  }

  .icon.tone-info {
    color: var(--chart-mem);
  }

  .separator {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.12);
  }
</style>
