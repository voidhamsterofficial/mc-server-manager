<script lang="ts">
  import type { ServerStatus } from "../api";
  import { STATUS_META } from "../status";

  interface Props {
    status: ServerStatus;
    showLabel?: boolean;
  }

  let { status, showLabel = false }: Props = $props();

  const meta = $derived(STATUS_META[status]);
</script>

{#if showLabel}
  <span
    class="pill"
    style:--blob-color="var({meta.colorVar})"
    style:--blob-soft="var({meta.softVar})"
  >
    <span class="dot" class:pulsing={meta.pulsing} class:glowing={status === "running"}></span>
    <span class="pill-text">{meta.label} {meta.emoji}</span>
  </span>
{:else}
  <span
    class="dot lone"
    class:pulsing={meta.pulsing}
    class:glowing={status === "running"}
    style:--blob-color="var({meta.colorVar})"
    title={meta.label}
  ></span>
{/if}

<style>
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--blob-color);
    flex-shrink: 0;
  }

  .dot.lone {
    display: inline-block;
    vertical-align: middle;
  }

  .dot.pulsing {
    animation: pulse 1.1s ease-in-out infinite;
  }

  .dot.glowing {
    box-shadow: 0 0 8px var(--blob-color);
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.45em;
    font-size: 0.82rem;
    font-weight: 700;
    line-height: 1;
    color: var(--blob-color);
    background: var(--blob-soft);
    padding: 0.42em 0.85em 0.42em 0.65em;
    border-radius: 999px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .pill-text {
    line-height: 1;
  }

  @keyframes pulse {
    0%,
    100% {
      transform: scale(1);
      opacity: 1;
    }
    50% {
      transform: scale(1.3);
      opacity: 0.6;
    }
  }
</style>
