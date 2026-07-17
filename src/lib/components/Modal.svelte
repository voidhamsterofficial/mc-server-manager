<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  interface Props {
    open: boolean;
    title: string;
    /** Omit to make the modal non-dismissable (e.g. while working). */
    onclose?: () => void;
    /** Wider dialog for content like tile grids. */
    wide?: boolean;
    children: Snippet;
  }

  let { open, title, onclose, wide = false, children }: Props = $props();

  function handleOverlayClick() {
    onclose?.();
  }
</script>

{#if open}
  <div
    class="overlay"
    transition:fade={{ duration: 140 }}
    onclick={handleOverlayClick}
    role="presentation"
  >
    <div
      class="dialog"
      class:wide
      transition:scale={{ start: 0.97, duration: 140, easing: cubicOut }}
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.key === "Escape" && onclose?.()}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      tabindex="-1"
    >
      <header>
        <h2>{title}</h2>
        {#if onclose}
          <button class="close" onclick={onclose} aria-label="Close">Close</button>
        {/if}
      </header>
      {@render children()}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(30, 20, 60, 0.35);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    z-index: 50;
  }

  .dialog {
    background: var(--surface);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-pop);
    padding: 1.5rem 1.75rem;
    width: min(480px, calc(100vw - 3rem));
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
  }

  .dialog.wide {
    width: min(680px, calc(100vw - 3rem));
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.25rem;
  }

  /* Blocky "Close" button — same bevel language as the shared Button. */
  .close {
    border: none;
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-family: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    line-height: 1;
    padding: 0.55em 1em;
    border-radius: 8px;
    cursor: pointer;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.25),
      inset 0 -3px 0 rgba(0, 0, 0, 0.18),
      0 0 0 2px rgba(20, 12, 38, 0.25);
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .close:hover {
    background: color-mix(in srgb, var(--accent-soft) 82%, var(--accent));
  }

  .close:active {
    filter: brightness(0.94);
  }
</style>
