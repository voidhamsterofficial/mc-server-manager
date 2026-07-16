<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  interface Props {
    open: boolean;
    title: string;
    /** Omit to make the modal non-dismissable (e.g. while working). */
    onclose?: () => void;
    children: Snippet;
  }

  let { open, title, onclose, children }: Props = $props();

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
          <button class="close" onclick={onclose} aria-label="Close">✕</button>
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

  .close {
    border: none;
    background: var(--surface-2);
    color: var(--muted);
    width: 32px;
    height: 32px;
    border-radius: 50%;
    cursor: pointer;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .close:hover {
    background: var(--border);
    color: var(--text);
  }
</style>
