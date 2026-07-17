<script lang="ts">
  import { api, type ServerAddress, type ServerConfig, type ForwardResult } from "../../api";
  import { serversStore } from "../../stores/servers.svelte";
  import { statsStore } from "../../stores/stats.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatBytes, formatDateTime, formatUptime } from "../../format";
  import { STATUS_META } from "../../status";
  import StatTile from "../../components/StatTile.svelte";
  import Sparkline from "../../components/Sparkline.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  const BYTES_PER_MB = 1024 * 1024;

  const status = $derived(serversStore.statusOf(server.id));
  const statusMeta = $derived(STATUS_META[status]);
  const players = $derived(serversStore.playersOf(server.id));
  const stats = $derived(statsStore.of(server.id));
  const isLive = $derived(stats.latest !== null);
  const isProxy = $derived(server.loader === "velocity" || server.loader === "bungeecord");

  let address = $state<ServerAddress | null>(null);

  $effect(() => {
    api
      .getServerAddress(server.id)
      .then((result) => (address = result))
      .catch(() => (address = null));
  });

  const lanAddress = $derived(address ? `${address.lanIp}:${address.port}` : "—");

  // Port forwarding (UPnP). `forward` holds the last attempt's outcome.
  let forward = $state<ForwardResult | null>(null);
  let forwardBusy = $state(false);
  const forwarded = $derived(forward?.success ?? false);

  async function openToInternet() {
    forwardBusy = true;
    try {
      forward = await api.openPortForward(server.id);
      if (forward.success && !forward.cgnat) {
        toastsStore.success("Port forwarded — friends can join 🌍");
      }
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      forwardBusy = false;
    }
  }

  async function closeToInternet() {
    forwardBusy = true;
    try {
      await api.closePortForward(server.id);
      forward = null;
      toastsStore.show("Closed to the internet 🔒");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      forwardBusy = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toastsStore.success("Copied to clipboard 📋");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  // Fixed memory scale relative to the configured heap keeps the sparkline
  // stable; the JVM can exceed -Xmx with off-heap memory, hence the headroom.
  const memoryScaleMax = $derived(server.memoryMb * BYTES_PER_MB * 1.5);

  const cpuText = $derived(isLive ? `${stats.latest!.cpuPercent.toFixed(1)} %` : "—");
  const memoryText = $derived(isLive ? formatBytes(stats.latest!.memoryBytes) : "—");
  const uptimeText = $derived(isLive ? formatUptime(stats.latest!.uptimeSeconds) : "—");
  const createdText = $derived(formatDateTime(server.createdAtUnix));
</script>

<div class="dash">
  {#if !isProxy}
    <button class="address" onclick={() => copy(lanAddress)} title="Click to copy">
      <span class="address-label">🔗 LAN address — click to copy</span>
      <span class="address-value">{lanAddress}</span>
    </button>

    <div
      class="forward"
      class:ok={forwarded && !forward?.cgnat}
      class:warn={forwarded && forward?.cgnat}
      class:err={forward !== null && !forward.success}
    >
      <div class="forward-head">
        <div class="forward-title">
          <span class="forward-label">🌍 Play over the internet</span>
          {#if forwarded}<span class="forward-tag">forwarded</span>{/if}
        </div>
        {#if forwarded}
          <Button variant="soft" disabled={forwardBusy} onclick={closeToInternet}>
            {forwardBusy ? "Working…" : "Close"}
          </Button>
        {:else}
          <Button disabled={forwardBusy} onclick={openToInternet}>
            {forwardBusy ? "Opening…" : "Open to internet"}
          </Button>
        {/if}
      </div>

      {#if forward === null}
        <p class="forward-hint">
          Ask your router to forward this server's port automatically (UPnP) so friends can
          join from anywhere. If it can't, the Docs explain how to forward it manually.
        </p>
      {:else}
        <p class="forward-message">{forward.message}</p>
        {#if forward.publicAddress}
          <button
            class="pub-address"
            onclick={() => copy(forward!.publicAddress!)}
            title="Click to copy"
          >
            <span class="address-label">🔗 Public address — click to copy</span>
            <span class="address-value">{forward.publicAddress}</span>
          </button>
        {/if}
      {/if}
    </div>
  {/if}

  <div class="grid">
    <StatTile label="Status" value="{statusMeta.label} {statusMeta.emoji}" />
    <StatTile
      label="Players online"
      value={String(players.length)}
      sub={players.length > 0 ? players.slice(0, 5).join(", ") : "nobody here yet 🌙"}
    />
    <StatTile label="Uptime" value={uptimeText} />
    <StatTile label="Version" value={server.mcVersion} sub={server.loader} />
    <StatTile
      label="CPU"
      value={cpuText}
      sub={isLive ? "of the whole machine" : "start the server to see stats"}
    >
      {#if stats.cpuHistory.length > 1}
        <Sparkline values={stats.cpuHistory} max={100} color="var(--chart-cpu)" />
      {/if}
    </StatTile>
    <StatTile label="Memory" value={memoryText} sub="allocated: {server.memoryMb} MB">
      {#if stats.memoryHistory.length > 1}
        <Sparkline values={stats.memoryHistory} max={memoryScaleMax} color="var(--chart-mem)" />
      {/if}
    </StatTile>
    <StatTile label="Created" value={createdText} />
  </div>
</div>

<style>
  .dash {
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
    padding: 0.25rem 0.25rem 1rem;
  }

  .address {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    align-items: flex-start;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 0.9rem 1.1rem;
    cursor: pointer;
    font-family: inherit;
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  .address:hover {
    border-color: color-mix(in srgb, var(--border) 40%, var(--accent));
  }

  .address-label {
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .address-value {
    font-family: var(--font-pixel);
    font-size: 1.35rem;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 1.2rem;
  }

  .forward {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 4px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 0.9rem 1.1rem;
  }

  .forward.ok {
    border-left-color: var(--mint);
  }

  .forward.warn {
    border-left-color: var(--peach);
  }

  .forward.err {
    border-left-color: var(--strawberry);
  }

  .forward-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .forward-title {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .forward-label {
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .forward-tag {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--mint);
    background: var(--mint-soft);
    border-radius: var(--radius-sm);
    padding: 0.15em 0.6em;
  }

  .forward-hint,
  .forward-message {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
    line-height: 1.45;
  }

  .pub-address {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    align-items: flex-start;
    text-align: left;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 0.7rem 0.9rem;
    cursor: pointer;
    font-family: inherit;
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  .pub-address:hover {
    border-color: color-mix(in srgb, var(--border) 40%, var(--accent));
  }
</style>
