<script lang="ts">
  import { fly } from "svelte/transition";
  import { CircleCheckBig, OctagonAlert, Info } from "@lucide/svelte";
  import { toastsStore } from "../stores/toasts.svelte";

  const KIND_ICON = { success: CircleCheckBig, error: OctagonAlert, info: Info } as const;
</script>

<div class="stack" aria-live="polite">
  {#each toastsStore.toasts as toast (toast.id)}
    {@const Icon = KIND_ICON[toast.kind]}
    <button
      class="toast {toast.kind}"
      transition:fly={{ y: 8, duration: 150 }}
      onclick={() => toastsStore.dismiss(toast.id)}
    >
      <span class="icon"><Icon size={18} /></span>
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

  .icon {
    display: inline-flex;
    flex-shrink: 0;
  }

  .toast.success .icon {
    color: var(--mint);
  }

  .toast.error .icon {
    color: var(--strawberry);
  }
</style>
