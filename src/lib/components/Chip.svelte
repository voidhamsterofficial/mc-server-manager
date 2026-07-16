<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<span class="chip">{@render children()}</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    font-size: 0.78rem;
    font-weight: 600;
    line-height: 1;
    color: var(--muted);
    background: var(--surface-2);
    border-radius: 999px;
    padding: 0.45em 0.85em;
    text-transform: capitalize;
    white-space: nowrap;
  }
</style>
