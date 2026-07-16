<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "primary" | "soft" | "danger" | "ghost";
    disabled?: boolean;
    title?: string;
    type?: "button" | "submit";
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = "primary",
    disabled = false,
    title,
    type = "button",
    onclick,
    children,
  }: Props = $props();
</script>

<button {type} class="btn {variant}" {disabled} {title} {onclick}>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4em;
    line-height: 1;
    font-family: inherit;
    font-size: 0.95rem;
    font-weight: 600;
    border: none;
    border-radius: 999px;
    padding: 0.65em 1.25em;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out),
      opacity var(--duration-fast) var(--ease-out);
  }

  .btn:active:not(:disabled) {
    filter: brightness(0.94);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Filled buttons get a classic blocky bevel: light top edge, dark bottom
     edge, chunky outline — feedback stays color-only. */
  .primary,
  .soft,
  .danger {
    border-radius: 8px;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.25),
      inset 0 -3px 0 rgba(0, 0, 0, 0.18),
      0 0 0 2px rgba(20, 12, 38, 0.25);
  }

  .primary {
    background: var(--accent);
    color: var(--on-accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }

  .soft {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .soft:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent-soft) 82%, var(--accent));
  }

  .danger {
    background: var(--strawberry-soft);
    color: var(--strawberry);
  }

  .danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--strawberry-soft) 82%, var(--strawberry));
  }

  .ghost {
    background: transparent;
    color: var(--muted);
  }

  .ghost:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
</style>
