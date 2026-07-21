<script lang="ts">
  // The shared single-line text dialog: kick/ban reasons, and names for new
  // or renamed files. Driven by textPromptStore.ask().
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { textPromptStore } from "../stores/textPrompt.svelte";

  function submit(event: SubmitEvent) {
    event.preventDefault();
    textPromptStore.confirm();
  }

  /** Focus the field as soon as the dialog opens. */
  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
</script>

<Modal
  open={textPromptStore.open}
  title={textPromptStore.title}
  onclose={() => textPromptStore.cancel()}
>
  <form onsubmit={submit}>
    <input
      class="text-field"
      type="text"
      bind:value={textPromptStore.value}
      placeholder={textPromptStore.placeholder}
      spellcheck="false"
      use:autofocus
    />
    {#if textPromptStore.hint !== null}
      <p class="hint">{textPromptStore.hint}</p>
    {/if}
    <div class="actions">
      <Button variant="ghost" onclick={() => textPromptStore.cancel()}>Cancel</Button>
      <Button
        type="submit"
        variant={textPromptStore.variant}
        disabled={textPromptStore.required && textPromptStore.value.trim() === ""}
      >
        {textPromptStore.actionLabel}
      </Button>
    </div>
  </form>
</Modal>

<style>
  .text-field {
    width: 100%;
    font-family: inherit;
    font-size: 0.95rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.55em 0.9em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  .text-field:focus {
    border-color: var(--accent);
  }

  .hint {
    margin: 0.5rem 0 1rem;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .actions {
    margin-top: 1rem;
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
