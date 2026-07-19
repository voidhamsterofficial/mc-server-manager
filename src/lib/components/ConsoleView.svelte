<script lang="ts">
  // Word-wrapped, color-aware console. `content-visibility: auto` lets the
  // browser skip layout/paint for offscreen lines, so the full 5000-line
  // buffer stays smooth without manual virtualization.

  import { onMount } from "svelte";
  import { Copy, FileText, Terminal } from "@lucide/svelte";
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

  let viewport = $state<HTMLDivElement | null>(null);
  let stickToBottom = $state(true);

  function handleScroll() {
    if (!viewport) {
      return;
    }
    const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    stickToBottom = distanceFromBottom < 40;
  }

  function scrollToBottom() {
    if (!viewport) {
      return;
    }
    // Double rAF: content-visibility means heights settle a frame late, so
    // scrolling immediately would land short of the true bottom.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (viewport) {
          viewport.scrollTop = viewport.scrollHeight;
        }
      });
    });
  }

  // Jump to the newest output the moment the console is shown, even when no
  // new line arrives after navigation.
  onMount(scrollToBottom);

  $effect(() => {
    // Follow new output unless the user scrolled up to read history.
    void lines.length;
    if (stickToBottom) {
      scrollToBottom();
    }
  });
</script>

<div
  class="viewport"
  bind:this={viewport}
  onscroll={handleScroll}
  oncontextmenu={openMenu}
  role="log"
  aria-label="Server console output"
>
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

<style>
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
