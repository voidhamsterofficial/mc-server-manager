<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Tints the chip's icon/text so at-a-glance meaning survives without
     *  relying on emoji for color. Omit for a plain neutral chip. */
    tone?: "success" | "warning" | "info";
    children: Snippet;
  }

  let { tone, children }: Props = $props();
</script>

<span class="chip" class:tone-success={tone === "success"} class:tone-warning={tone === "warning"} class:tone-info={tone === "info"}>
  {@render children()}
</span>

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
    border-radius: var(--radius-sm);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.12);
    padding: 0.45em 0.85em;
    text-transform: capitalize;
    white-space: nowrap;
  }

  .tone-success {
    color: var(--mint);
    background: var(--mint-soft);
  }

  .tone-warning {
    color: var(--peach);
    background: var(--peach-soft);
  }

  .tone-info {
    color: var(--chart-mem);
    background: var(--surface-2);
  }
</style>
