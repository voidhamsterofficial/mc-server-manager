<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    value: string;
    sub?: string;
    /** Optional chart area rendered under the value (e.g. a Sparkline). */
    children?: Snippet;
  }

  let { label, value, sub, children }: Props = $props();
</script>

<div class="tile">
  <span class="label">{label}</span>
  <span class="value">{value}</span>
  {#if sub}
    <span class="sub">{sub}</span>
  {/if}
  {#if children}
    <div class="chart">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .tile {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1.2rem 1.35rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }

  .label {
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .value {
    font-family: var(--font-pixel);
    font-size: 1.35rem;
    font-weight: 700;
    line-height: 1.3;
    color: var(--text);
  }

  .sub {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .chart {
    margin-top: 0.45rem;
  }
</style>
