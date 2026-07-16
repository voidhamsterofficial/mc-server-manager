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
      transform 0.18s var(--ease-bounce),
      box-shadow 0.18s ease,
      background-color 0.18s ease,
      opacity 0.18s ease;
  }

  .btn:hover:not(:disabled) {
    transform: translateY(-1px) scale(1.04);
    box-shadow: var(--shadow-soft);
  }

  .btn:active:not(:disabled) {
    transform: scale(0.94);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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

  .danger {
    background: var(--strawberry-soft);
    color: var(--strawberry);
  }

  .ghost {
    background: transparent;
    color: var(--muted);
  }

  .ghost:hover:not(:disabled) {
    color: var(--text);
    box-shadow: none;
  }
</style>
