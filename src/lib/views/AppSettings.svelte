<script lang="ts">
  import { fade } from "svelte/transition";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { api, type AppSettings, type JavaInstall } from "../api";
  import { toastsStore } from "../stores/toasts.svelte";
  import Button from "../components/Button.svelte";

  let settings = $state<AppSettings | null>(null);
  let javaInstalls = $state<JavaInstall[]>([]);
  let detectingJava = $state(false);

  $effect(() => {
    loadSettings();
    detectJava();
  });

  async function loadSettings() {
    try {
      settings = await api.getSettings();
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function browseBaseDir() {
    const picked = await openFolderDialog({
      directory: true,
      title: "Choose where new servers are created",
    });
    if (typeof picked !== "string") {
      return;
    }
    try {
      settings = await api.setServersBaseDir(picked);
      toastsStore.success("Default server location updated 📁");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function detectJava() {
    detectingJava = true;
    try {
      javaInstalls = await api.detectJava();
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      detectingJava = false;
    }
  }
</script>

<section class="settings" in:fade={{ duration: 120 }}>
  <h1>Settings ⚙️</h1>

  <div class="card">
    <div class="card-head">
      <h3>📁 Default server location</h3>
      <Button variant="soft" disabled={settings === null} onclick={browseBaseDir}>
        Change…
      </Button>
    </div>
    <p class="hint">
      New servers are created here. You can pick a different folder per server in the
      create wizard — the last choice becomes the new default. Existing servers stay
      where they are.
    </p>
    <code class="path">{settings?.serversBaseDir ?? "…"}</code>
  </div>

  <div class="card">
    <div class="card-head">
      <h3>☕ Java installations</h3>
      <Button variant="soft" disabled={detectingJava} onclick={detectJava}>
        {detectingJava ? "Scanning…" : "🔍 Re-scan"}
      </Button>
    </div>
    {#if detectingJava && javaInstalls.length === 0}
      <p class="hint">Looking for Java on this machine…</p>
    {:else if javaInstalls.length === 0}
      <p class="hint">
        No Java found. Install one (e.g. Temurin) — automatic downloads are coming soon!
      </p>
    {:else}
      <ul class="java-list">
        {#each javaInstalls as install (install.path)}
          <li>
            <span class="java-version">Java {install.majorVersion}</span>
            <code class="path">{install.path}</code>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .settings {
    max-width: 860px;
    margin: 0 auto;
    padding: 1.5rem 2rem 3rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  h1 {
    font-size: 1.5rem;
    margin: 0 0 0.25rem;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1rem 1.25rem;
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  h3 {
    margin: 0 0 0.4rem;
    font-size: 1rem;
  }

  .hint {
    margin: 0 0 0.6rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .path {
    display: block;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    padding: 0.45em 0.7em;
    overflow-wrap: break-word;
    word-break: break-all;
    user-select: text;
  }

  .java-list {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .java-list li {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .java-version {
    flex-shrink: 0;
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--accent-strong);
    background: var(--accent-soft);
    border-radius: var(--radius-sm);
    padding: 0.2em 0.7em;
  }

  .java-list .path {
    flex: 1;
    min-width: 0;
  }
</style>
