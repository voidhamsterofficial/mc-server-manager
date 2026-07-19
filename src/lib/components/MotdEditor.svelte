<script lang="ts">
  // MOTD editor in the style of the classic web MOTD creators: a two-line
  // text field with §-code buttons and a live, game-style preview
  // (obfuscated text scrambles just like in-game).

  import { onDestroy } from "svelte";
  import { MOTD_COLORS, MOTD_FORMATS, parseMotd } from "../util/motd";

  interface Props {
    /** Editor-form text (real § characters, real newlines). */
    value: string;
    onchange: (value: string) => void;
  }

  let { value, onchange }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  const OBFUSCATION_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789?!@#%&";

  const previewLines = $derived(parseMotd(value));
  const hasObfuscated = $derived(value.includes("§k"));

  // A ticking seed re-randomizes obfuscated glyphs a few times a second.
  let scrambleSeed = $state(0);
  const scrambleTimer = setInterval(() => {
    if (hasObfuscated) {
      scrambleSeed = (scrambleSeed + 1) % 100000;
    }
  }, 80);
  onDestroy(() => clearInterval(scrambleTimer));

  function scramble(text: string): string {
    void scrambleSeed;
    let out = "";
    for (const char of text) {
      out += char === " " ? " " : OBFUSCATION_CHARS[Math.floor(Math.random() * OBFUSCATION_CHARS.length)];
    }
    return out;
  }

  function insertCode(code: string) {
    const insertion = `§${code}`;
    const cursor = textarea?.selectionStart ?? value.length;
    const updated = value.slice(0, cursor) + insertion + value.slice(cursor);
    onchange(updated);

    requestAnimationFrame(() => {
      textarea?.focus();
      textarea?.setSelectionRange(cursor + insertion.length, cursor + insertion.length);
    });
  }

  function handleInput(event: Event) {
    // The MOTD is at most two lines.
    const raw = (event.target as HTMLTextAreaElement).value;
    const lines = raw.split("\n").slice(0, 2);
    onchange(lines.join("\n"));
  }
</script>

<div class="motd-editor">
  <div class="toolbar">
    <span class="swatches">
      {#each MOTD_COLORS as colorEntry (colorEntry.code)}
        <button
          type="button"
          class="swatch"
          style:background={colorEntry.hex}
          title="{colorEntry.name} (§{colorEntry.code})"
          onclick={() => insertCode(colorEntry.code)}
          aria-label={colorEntry.name}
        ></button>
      {/each}
    </span>
    <span class="formats">
      {#each MOTD_FORMATS as format (format.code)}
        <button
          type="button"
          class="format"
          title="{format.title} (§{format.code})"
          onclick={() => insertCode(format.code)}
        >
          {format.label}
        </button>
      {/each}
    </span>
  </div>

  <textarea
    bind:this={textarea}
    {value}
    rows="2"
    oninput={handleInput}
    placeholder={"§aA §lMinecraft §r§aServer\n§7Two lines supported!"}
    spellcheck="false"
  ></textarea>

  <div class="preview">
    {#each previewLines as line, lineIndex (lineIndex)}
      <div class="preview-line">
        {#if line.length === 0}
          <span>&nbsp;</span>
        {:else}
          {#each line as span, index (index)}
            <span
              style:color={span.color ?? "#AAAAAA"}
              class:bold={span.bold}
              class:italic={span.italic}
              class:underline={span.underline}
              class:strike={span.strike}
              class:obfuscated={span.obfuscated}>{span.obfuscated
                ? scramble(span.text)
                : span.text}</span
            >
          {/each}
        {/if}
      </div>
    {/each}
  </div>
  <p class="tip">Press Enter for a second line. Reset (§r or ⟲) before the line ends.</p>
</div>

<style>
  .motd-editor {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .swatches {
    display: flex;
    gap: 3px;
    flex-wrap: wrap;
  }

  .swatch {
    width: 20px;
    height: 20px;
    border: none;
    border-radius: 3px;
    cursor: pointer;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.25),
      inset 0 -2px 0 rgba(0, 0, 0, 0.25),
      0 0 0 1px rgba(15, 15, 18, 0.4);
  }

  .formats {
    display: flex;
    gap: 3px;
  }

  .format {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-pixel);
    font-size: 0.85rem;
    cursor: pointer;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.08),
      inset 0 -2px 0 rgba(0, 0, 0, 0.2);
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .format:hover {
    background: var(--accent-soft);
  }

  textarea {
    font-family: var(--font-mono);
    font-size: 0.9rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.55em 0.8em;
    outline: none;
    resize: none;
    transition: border-color 0.18s ease;
  }

  textarea:focus {
    border-color: var(--accent);
  }

  /* Server-list style preview: always dark, pixel font. */
  .preview {
    background: #1a1b1e;
    border-radius: var(--radius-sm);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.5);
    padding: 0.6rem 0.9rem;
    font-family: var(--font-pixel);
    font-size: 0.95rem;
    color: #aaaaaa;
    min-height: 2.6em;
  }

  .preview-line {
    white-space: pre-wrap;
    overflow-wrap: break-word;
    line-height: 1.35;
  }

  .bold {
    font-weight: 700;
  }

  .italic {
    font-style: italic;
  }

  .underline {
    text-decoration: underline;
  }

  .strike {
    text-decoration: line-through;
  }

  .underline.strike {
    text-decoration: underline line-through;
  }

  .obfuscated {
    opacity: 0.9;
  }

  .tip {
    margin: 0;
    font-size: 0.75rem;
    color: var(--muted);
  }
</style>
