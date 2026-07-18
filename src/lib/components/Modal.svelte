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

  let dialog = $state<HTMLDivElement | null>(null);
  let previouslyFocused: HTMLElement | null = null;

  $effect(() => {
    if (open) {
      previouslyFocused = document.activeElement as HTMLElement | null;
      // Defer past children that focus themselves (e.g. ReasonPrompt's input),
      // and don't steal focus if it's already landed inside the dialog.
      queueMicrotask(() => {
        if (dialog && !dialog.contains(document.activeElement)) {
          dialog.focus();
        }
      });
    } else if (previouslyFocused) {
      previouslyFocused.focus();
      previouslyFocused = null;
    }
  });

  function handleOverlayClick() {
    onclose?.();
  }

  /** Visible, enabled, focusable elements inside the dialog, in tab order. */
  function focusable(): HTMLElement[] {
    if (!dialog) return [];
    const selector =
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
    return Array.from(dialog.querySelectorAll<HTMLElement>(selector)).filter(
      (element) => element.offsetParent !== null,
    );
  }

  /** Escape to close, and keep Tab focus cycling within the open dialog. */
  function onWindowKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      onclose?.();
      return;
    }
    if (event.key !== "Tab") return;
    const items = focusable();
    if (items.length === 0) {
      event.preventDefault();
      dialog?.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === dialog)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if open}
  <div
    class="overlay"
    transition:fade={{ duration: 140 }}
    onclick={handleOverlayClick}
    role="presentation"
  >
    <!-- The click handler only stops backdrop clicks from bubbling out and
         closing the modal; keyboard interaction (Esc, Tab trap) is handled on
         the window, so no keydown handler belongs here. -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="dialog"
      class:wide
      bind:this={dialog}
      transition:scale={{ start: 0.97, duration: 140, easing: cubicOut }}
      onclick={(event) => event.stopPropagation()}
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
