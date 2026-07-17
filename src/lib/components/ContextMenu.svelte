<script lang="ts">
  // The custom right-click menu, styled like an in-game tooltip: dark
  // panel, chunky outline, pixel-font entries.

  import { fade } from "svelte/transition";
  import { contextMenuStore, type MenuItem } from "../stores/contextMenu.svelte";

  function run(item: MenuItem) {
    if (item.disabled) {
      return;
    }
    contextMenuStore.close();
    item.action();
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
        <button
          class="item"
          class:danger={entry.danger}
          disabled={entry.disabled}
          role="menuitem"
          onclick={() => run(entry)}
        >
          <span class="emoji">{entry.emoji ?? ""}</span>
          {entry.label}
        </button>
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

  .emoji {
    width: 1.2em;
    text-align: center;
    font-family: var(--font-body);
  }

  .separator {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.12);
  }
</style>
