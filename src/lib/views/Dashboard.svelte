<script lang="ts">
  import { fade } from "svelte/transition";
  import { Plus, FolderInput, RefreshCw, TriangleAlert, Globe, Archive, Puzzle } from "@lucide/svelte";
  import { serversStore } from "../stores/servers.svelte";
  import type { ServerConfig } from "../ipc/api";
  import ServerCard from "../components/ServerCard.svelte";
  import Button from "../components/Button.svelte";
  import GrassBlock from "../components/GrassBlock.svelte";
  import { FEATURE_COLOR } from "../util/features";

  interface Props {
    onopen: (serverId: string) => void;
    onnew: () => void;
    onimport: () => void;
    ondocs: () => void;
    onservermenu: (event: MouseEvent, server: ServerConfig) => void;
  }

  let { onopen, onnew, onimport, ondocs, onservermenu }: Props = $props();

  let retrying = $state(false);

  async function retry() {
    retrying = true;
    try {
      await serversStore.refresh();
    } catch {
      // serversStore.loadError already reflects the failure for the UI.
    } finally {
      retrying = false;
    }
  }
</script>

<section class="dashboard">
  {#if !serversStore.loaded}
    <div class="empty" in:fade={{ duration: 120 }}>
      <GrassBlock size={72} />
      <p>Loading your servers…</p>
    </div>
  {:else if serversStore.loadError}
    <div class="empty" in:fade={{ duration: 120 }}>
      <TriangleAlert size={48} color="var(--strawberry)" />
      <p>Couldn't load your servers.</p>
      <p class="error-detail">{serversStore.loadError}</p>
      <Button variant="soft" disabled={retrying} onclick={retry}>
        <RefreshCw size={15} /> {retrying ? "Retrying…" : "Try again"}
      </Button>
    </div>
  {:else if serversStore.servers.length === 0}
    <div class="welcome" in:fade={{ duration: 120 }}>
      <GrassBlock size={72} />
      <h1>Welcome to ServerForge!</h1>
      <p class="lede">
        Build a Minecraft server in a couple of clicks — ServerForge handles the software, the
        right Java, and the boring setup.
      </p>
      <div class="welcome-actions">
        <Button onclick={onnew}><Plus size={15} /> New server</Button>
        <Button variant="soft" onclick={onimport}><FolderInput size={15} /> Import existing</Button>
      </div>
      <div class="tips">
        <div class="tip">
          <Globe size={18} color={FEATURE_COLOR.dashboard} />
          <div>
            <strong>Play with friends</strong>
            <span>Open your server to the internet with one click — no router setup needed.</span>
          </div>
        </div>
        <div class="tip">
          <Archive size={18} color={FEATURE_COLOR.backups} />
          <div>
            <strong>Automatic backups</strong>
            <span>Schedule backups so a bad update or a griefer never costs you a world.</span>
          </div>
        </div>
        <div class="tip">
          <Puzzle size={18} color={FEATURE_COLOR.plugins} />
          <div>
            <strong>Mods &amp; plugins</strong>
            <span>Search and install straight from Modrinth and CurseForge.</span>
          </div>
        </div>
      </div>
      <button class="docs-link" onclick={ondocs}>Read the docs</button>
    </div>
  {:else}
    <h1>Your servers</h1>
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

  h1 {
    font-size: 1.5rem;
    margin: 0 0 1.25rem;
  }

  /* Always two per row, at any window width — the cards grow to fill the
     space rather than a third column appearing on a wide screen. */
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 1.4rem;
  }

  .empty {
    text-align: center;
    padding: 4rem 1rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.9rem;
    color: var(--muted);
  }

  .error-detail {
    max-width: 480px;
    font-size: 0.85rem;
    color: var(--muted);
    word-break: break-word;
  }

  .welcome {
    text-align: center;
    padding: 3.5rem 1rem 2.5rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    max-width: 640px;
    margin: 0 auto;
  }

  .welcome h1 {
    margin: 0;
    font-family: var(--font-pixel);
  }

  .lede {
    margin: 0;
    color: var(--muted);
    max-width: 460px;
  }

  .welcome-actions {
    display: flex;
    gap: 0.6rem;
    margin-top: 0.25rem;
  }

  .tips {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 1rem;
    width: 100%;
    margin-top: 1.5rem;
    text-align: left;
  }

  .tip {
    display: flex;
    gap: 0.6rem;
    align-items: flex-start;
    padding: 0.9rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .tip strong {
    display: block;
    font-size: 0.88rem;
    margin-bottom: 0.2rem;
  }

  .tip span {
    font-size: 0.8rem;
    color: var(--muted);
    line-height: 1.4;
  }

  .docs-link {
    margin-top: 0.5rem;
    border: none;
    background: none;
    color: var(--accent);
    font-family: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    padding: 0;
  }

  .docs-link:hover {
    text-decoration: underline;
  }
</style>
