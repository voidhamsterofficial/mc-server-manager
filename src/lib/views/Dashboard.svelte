<script lang="ts">
  import { fade } from "svelte/transition";
  import { serversStore } from "../stores/servers.svelte";
  import ServerCard from "../components/ServerCard.svelte";
  import Button from "../components/Button.svelte";
  import CreateServerWizard from "./CreateServerWizard.svelte";

  interface Props {
    onopen: (serverId: string) => void;
  }

  let { onopen }: Props = $props();

  let wizardOpen = $state(false);
</script>

<section class="dashboard">
  <div class="head">
    <h1>Your servers</h1>
    <Button onclick={() => (wizardOpen = true)}>＋ New server</Button>
  </div>

  {#if serversStore.servers.length === 0}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="egg">🥚</span>
      <p>No servers yet — let's hatch your first one!</p>
      <Button onclick={() => (wizardOpen = true)}>Create a server</Button>
    </div>
  {:else}
    <div class="grid">
      {#each serversStore.servers as server (server.id)}
        <ServerCard {server} onopen={() => onopen(server.id)} />
      {/each}
    </div>
  {/if}
</section>

<CreateServerWizard open={wizardOpen} onclose={() => (wizardOpen = false)} />

<style>
  .dashboard {
    max-width: 1240px;
    margin: 0 auto;
    padding: 1.5rem 2rem 3rem;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.25rem;
  }

  h1 {
    font-size: 1.5rem;
    margin: 0;
  }

  /* auto-fit collapses unused tracks, so few servers get comfortably wide
     cards (up to 560px) instead of huddling in narrow slots. The 320px
     minimum guarantees two columns at the default window size. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 560px));
    gap: 1.4rem;
  }

  .empty {
    text-align: center;
    padding: 4rem 1rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    color: var(--muted);
  }

  .egg {
    font-size: 3.5rem;
    display: inline-block;
    animation: bounce 2.2s ease-in-out infinite;
  }

  @keyframes bounce {
    0%,
    100% {
      transform: translateY(0) rotate(-4deg);
    }
    50% {
      transform: translateY(-10px) rotate(4deg);
    }
  }
</style>
