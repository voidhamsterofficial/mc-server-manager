<script lang="ts">
  import { fade } from "svelte/transition";
  import { serversStore } from "../stores/servers.svelte";
  import type { ServerConfig } from "../api";
  import ServerCard from "../components/ServerCard.svelte";
  import Button from "../components/Button.svelte";

  interface Props {
    onopen: (serverId: string) => void;
    onnew: () => void;
    onservermenu: (event: MouseEvent, server: ServerConfig) => void;
  }

  let { onopen, onnew, onservermenu }: Props = $props();
</script>

<section class="dashboard">
  <div class="head">
    <h1>Your servers</h1>
    <Button onclick={onnew}>＋ New server</Button>
  </div>

  {#if serversStore.servers.length === 0}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="egg">🥚</span>
      <p>No servers yet — let's hatch your first one!</p>
      <Button onclick={onnew}>Create a server</Button>
    </div>
  {:else}
    <div class="grid">
      {#each serversStore.servers as server (server.id)}
        <ServerCard
          {server}
          onopen={() => onopen(server.id)}
          oncontextmenu={(event) => onservermenu(event, server)}
        />
      {/each}
    </div>
  {/if}
</section>

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

  /* auto-fit + a 1fr max counts columns from the 320px minimum (a definite
     max like 560px would count from the max and waste a whole column of
     space); the card itself caps its width so a lone server isn't a
     screen-wide slab. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
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
