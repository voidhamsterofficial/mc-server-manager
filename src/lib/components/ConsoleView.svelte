<script lang="ts">
  // Word-wrapped, color-aware console. `content-visibility: auto` lets the
  // browser skip layout/paint for offscreen lines, so the full 5000-line
  // buffer stays smooth without manual virtualization.

  import type { ConsoleLine } from "../events";

  interface Props {
    lines: ConsoleLine[];
  }

  let { lines }: Props = $props();

  let viewport = $state<HTMLDivElement | null>(null);
  let stickToBottom = $state(true);

  function handleScroll() {
    if (!viewport) {
      return;
    }
    const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    stickToBottom = distanceFromBottom < 40;
  }

  $effect(() => {
    // Follow new output unless the user scrolled up to read history.
    void lines.length;
    if (stickToBottom && viewport) {
      viewport.scrollTop = viewport.scrollHeight;
    }
  });
</script>

<div class="viewport" bind:this={viewport} onscroll={handleScroll}>
  {#if lines.length === 0}
    <p class="empty">Console output will appear here… 🌱</p>
  {:else}
    {#each lines as line, index (index)}
      <div class="line {line.level}">
        {#each line.spans as span, spanIndex (spanIndex)}
          <span style:color={span.color ?? null} class:bold={span.bold}>{span.text}</span>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  /* The console is always terminal-dark, in both app themes, so authentic
     Minecraft/ANSI colors stay readable. */
  .viewport {
    height: 100%;
    overflow-y: auto;
    background: #1d1926;
    border-radius: var(--radius-md);
    padding: 0.6rem 0;
  }

  .line {
    padding: 1px 0.9rem;
    min-height: 1.4em;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    word-break: break-word;
    color: #d6cfe8;
    user-select: text;
    content-visibility: auto;
    contain-intrinsic-size: auto 19px;
  }

  .line.warn {
    color: #ffb454;
  }

  .line.error {
    color: #ff6b81;
  }

  .bold {
    font-weight: 700;
  }

  .empty {
    color: #8b84a3;
    text-align: center;
    margin-top: 2.5rem;
    font-size: 0.95rem;
  }
</style>
