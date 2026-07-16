<script lang="ts">
  import { fly } from "svelte/transition";
  import { toastsStore } from "../stores/toasts.svelte";

  const KIND_EMOJI = { success: "🎉", error: "😿", info: "💡" } as const;
</script>

<div class="stack" aria-live="polite">
  {#each toastsStore.toasts as toast (toast.id)}
    <button
      class="toast {toast.kind}"
      transition:fly={{ y: 8, duration: 150 }}
      onclick={() => toastsStore.dismiss(toast.id)}
    >
      <span class="emoji">{KIND_EMOJI[toast.kind]}</span>
      {toast.message}
    </button>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    bottom: 1.25rem;
    right: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    z-index: 100;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 0.6em;
    font-family: inherit;
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-pop);
    padding: 0.7em 1em;
    max-width: 340px;
    cursor: pointer;
    text-align: left;
  }

  .toast.success {
    border-color: var(--mint);
  }

  .toast.error {
    border-color: var(--strawberry);
  }

  .emoji {
    font-size: 1.1rem;
  }
</style>
