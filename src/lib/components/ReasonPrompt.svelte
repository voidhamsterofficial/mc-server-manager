<script lang="ts">
  // The shared kick/ban reason dialog. Driven by reasonPromptStore.ask().
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { reasonPromptStore } from "../stores/reasonPrompt.svelte";

  function submit(event: SubmitEvent) {
    event.preventDefault();
    reasonPromptStore.confirm();
  }

  /** Focus the field as soon as the dialog opens. */
  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
</script>

<Modal
  open={reasonPromptStore.open}
  title={reasonPromptStore.title}
  onclose={() => reasonPromptStore.cancel()}
>
  <form onsubmit={submit}>
    <input
      class="reason"
      type="text"
      bind:value={reasonPromptStore.value}
      placeholder={reasonPromptStore.placeholder}
      spellcheck="false"
      use:autofocus
    />
    <p class="hint">Leave blank to record no reason.</p>
    <div class="actions">
      <Button variant="ghost" onclick={() => reasonPromptStore.cancel()}>Cancel</Button>
      <Button type="submit" variant={reasonPromptStore.variant}>
        {reasonPromptStore.actionLabel}
      </Button>
    </div>
  </form>
</Modal>

<style>
  .reason {
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

  .reason:focus {
    border-color: var(--accent);
  }

  .hint {
    margin: 0.5rem 0 1rem;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
