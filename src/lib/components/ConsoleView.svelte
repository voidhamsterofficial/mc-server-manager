<script lang="ts">
  // Word-wrapped, color-aware console. `content-visibility: auto` lets the
  // browser skip layout/paint for offscreen lines, so the full 5000-line
  // buffer stays smooth without manual virtualization.

  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { ArrowDown, Copy, FileText, Terminal } from "@lucide/svelte";
  import type { ConsoleLine } from "../ipc/events";
  import { contextMenuStore, type MenuEntry } from "../stores/contextMenu.svelte";
  import { toastsStore } from "../stores/toasts.svelte";

  interface Props {
    lines: ConsoleLine[];
  }

  let { lines }: Props = $props();

  /** The whole buffer as plain text, one line per row. */
  function consoleText(): string {
    return lines.map((line) => line.spans.map((span) => span.text).join("")).join("\n");
  }

  async function copyText(text: string, successMessage: string) {
    if (text === "") {
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      toastsStore.success(successMessage);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  function openMenu(event: MouseEvent) {
    const selection = window.getSelection()?.toString() ?? "";
    const entries: MenuEntry[] = [
      {
        label: "Copy selection",
        icon: Copy,
        disabled: selection === "",
        action: () => copyText(selection, "Copied selection"),
      },
      {
        label: "Copy all output",
        icon: FileText,
        disabled: lines.length === 0,
        action: () => copyText(consoleText(), "Copied console output"),
      },
    ];
    contextMenuStore.show(event, entries);
  }

  /** Within this many pixels of the bottom still counts as "at the bottom",
   *  so a stray pixel of rounding doesn't drop the user out of following. */
  const BOTTOM_THRESHOLD_PX = 40;

  let viewport = $state<HTMLDivElement | null>(null);
  let content = $state<HTMLDivElement | null>(null);
  let stickToBottom = $state(true);

  function distanceFromBottom(element: HTMLDivElement): number {
    return element.scrollHeight - element.scrollTop - element.clientHeight;
  }

  function handleScroll() {
    if (!viewport) {
      return;
    }
    stickToBottom = distanceFromBottom(viewport) < BOTTOM_THRESHOLD_PX;
  }

  function pinToBottom() {
    if (!viewport) {
      return;
    }
    viewport.scrollTop = viewport.scrollHeight;
  }

  /** Explicit "jump to latest": resumes following as well as scrolling. */
  function jumpToBottom() {
    stickToBottom = true;
    pinToBottom();
  }

  onMount(() => {
    if (!content || !viewport) {
      return;
    }
    // Height-driven rather than line-count-driven. `content-visibility: auto`
    // means a line's real height isn't known until it's near the viewport, so
    // the buffer keeps growing for several frames after the lines exist —
    // scrolling once on mount (even after a rAF or two) lands short of the
    // true bottom on a full 5000-line buffer. Re-pinning on every height
    // change follows it down however long it takes to settle. The viewport is
    // watched as well as the content, so resizing the window — which reflows
    // wrapped lines — keeps the newest output in view too.
    const observer = new ResizeObserver(() => {
      if (stickToBottom) {
        pinToBottom();
      }
    });
    observer.observe(content);
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  $effect(() => {
    // Also follow on new output, because the observer alone isn't enough:
    // once the buffer is at its cap, every appended line drops one off the
    // front, so the content height is unchanged and no resize fires — while
    // the text underneath has shifted up by a line. Without this the view
    // silently drifts off the newest output on exactly the long-running
    // servers where following matters most.
    void lines.length;
    if (stickToBottom) {
      pinToBottom();
    }
  });
</script>

<div class="console-view">
  <div
    class="viewport"
    bind:this={viewport}
    onscroll={handleScroll}
    oncontextmenu={openMenu}
    role="log"
    aria-label="Server console output"
  >
    <div class="content" bind:this={content}>
      {#if lines.length === 0}
        <p class="empty"><Terminal size={16} /> Console output will appear here…</p>
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
  </div>

  {#if !stickToBottom}
    <button class="jump" onclick={jumpToBottom} transition:fade={{ duration: 120 }}>
      <ArrowDown size={15} /> Jump to latest
    </button>
  {/if}
</div>

<style>
  .console-view {
    position: relative;
    height: 100%;
  }

  /* The console is always terminal-dark, in both app themes, so authentic
     Minecraft/ANSI colors stay readable. */
  .viewport {
    height: 100%;
    overflow-y: auto;
    background: #1a1b1e;
    border-radius: var(--radius-md);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.5);
    padding: 0.6rem 0;
  }

  /* The scroll-height source the ResizeObserver watches. */
  .content {
    display: flow-root;
  }

  .line {
    padding: 1px 0.9rem;
    min-height: 1.4em;
    font-family: var(--font-pixel);
    font-size: 13px;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    word-break: break-word;
    color: #d8d8dc;
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

  /* Floats over the output, clear of the horizontal centre so it never sits
     on top of the newest line the user is trying to read. */
  .jump {
    position: absolute;
    right: 0.9rem;
    bottom: 0.8rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-family: inherit;
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.4rem 0.7rem;
    cursor: pointer;
  }

  .jump:hover {
    border-color: var(--accent);
    color: var(--accent-strong);
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: #9a9aa2;
    text-align: center;
    margin-top: 2.5rem;
    font-size: 0.95rem;
  }
</style>
