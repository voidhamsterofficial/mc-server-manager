<script lang="ts">
  // The shared confirmation dialog. Driven by confirmStore.ask().
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { confirmStore } from "../stores/confirm.svelte";
</script>

<Modal open={confirmStore.open} title={confirmStore.title} onclose={() => confirmStore.cancel()}>
  <p class="body">{confirmStore.body}</p>
  <div class="actions">
    <Button variant="ghost" onclick={() => confirmStore.cancel()}>Cancel</Button>
    {#if confirmStore.secondaryLabel !== null}
      <Button
        variant={confirmStore.secondaryVariant}
        onclick={() => confirmStore.chooseSecondary()}
      >
        {confirmStore.secondaryLabel}
      </Button>
    {/if}
    <Button variant={confirmStore.variant} onclick={() => confirmStore.confirm()}>
      {confirmStore.confirmLabel}
    </Button>
  </div>
</Modal>

<style>
  .body {
    margin: 0 0 1.25rem;
    font-size: 0.92rem;
    line-height: 1.5;
    color: var(--muted);
    /* Honours blank lines so a longer explanation can be split into
       paragraphs; ordinary wrapping is unaffected. */
    white-space: pre-line;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    /* Three buttons with wordy labels don't fit one line in a narrow dialog. */
    flex-wrap: wrap;
  }
</style>
