<script lang="ts">
  import { Link2, Globe } from "@lucide/svelte";
  import { api, type ServerAddress, type ServerConfig } from "../../ipc/api";
  import { serversStore } from "../../stores/servers.svelte";
  import { statsStore } from "../../stores/stats.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { portForwardStore } from "../../stores/portForward.svelte";
  import { serverAddressStore } from "../../stores/serverAddress.svelte";
  import { formatMemory, formatDateTime, formatUptime } from "../../util/format";
  import { STATUS_META } from "../../util/status";
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

  let address = $state<ServerAddress | null>(null);
  // Bumped per load so a slow reply for a server we've navigated away from
  // can't overwrite the one on screen.
  let addressToken = 0;
  /** Which server the shown address belongs to. */
  let addressServerId: string | null = null;

  $effect(() => {
    const serverId = server.id;
    // Depended on so the address refetches when the port is changed — from
    // the Settings tab, or from the prompt shown when a start hits a port
    // clash. Neither touches any other state this effect reads.
    serverAddressStore.revisionOf(serverId);

    const token = ++addressToken;
    // Clear only when the server changed: holding the previous server's
    // address while this one loads would show the wrong one. Re-reading the
    // same server's address after a port change must not blank the field to
    // "—" and back — it just updates in place when the new value lands.
    if (serverId !== addressServerId) {
      addressServerId = serverId;
      address = null;
    }

    api
      .getServerAddress(serverId)
      .then((result) => {
        if (token === addressToken) {
          address = result;
        }
      })
      .catch(() => {
        if (token === addressToken) {
          address = null;
        }
      });
  });

  const lanAddress = $derived(address ? `${address.lanIp}:${address.port}` : "—");

  // Port forwarding (UPnP) — kept per server in a store, since this component
  // is reused as you switch servers.
  const forward = $derived(portForwardStore.resultOf(server.id));
  const forwardBusy = $derived(portForwardStore.isBusy(server.id));
  const forwarded = $derived(forward?.success ?? false);

  $effect(() => {
    // Mappings outlive the app, so ask the router whether this server is
    // already forwarded rather than assuming it isn't. Once per server per
    // session; failures just leave it showing "not forwarded".
    const serverId = server.id;
    if (!portForwardStore.claimStatusCheck(serverId)) {
      return;
    }
    api
      .portForwardStatus(serverId)
      .then((result) => {
        if (result) {
          portForwardStore.record(serverId, result);
        }
      })
      .catch(() => {});
  });

  async function openToInternet() {
    // Pin the id: navigating away mid-request must not file the result under
    // whichever server is on screen when it returns.
    const serverId = server.id;
    portForwardStore.setBusy(serverId, true);
    try {
      const result = await api.openPortForward(serverId);
      portForwardStore.record(serverId, result);
      if (result.success && !result.cgnat) {
        toastsStore.success("Port forwarded — friends can join");
      }
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      portForwardStore.setBusy(serverId, false);
    }
  }

  async function closeToInternet() {
    const serverId = server.id;
    portForwardStore.setBusy(serverId, true);
    try {
      await api.closePortForward(serverId);
      portForwardStore.clear(serverId);
      toastsStore.show("Closed to the internet");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      portForwardStore.setBusy(serverId, false);
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toastsStore.success("Copied to clipboard");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  // Fixed memory scale relative to the configured heap keeps the sparkline
  // stable; the JVM can exceed -Xmx with off-heap memory, hence the headroom.
  const memoryScaleMax = $derived(server.memoryMb * BYTES_PER_MB * 1.5);

  const cpuText = $derived(isLive ? `${stats.latest!.cpuPercent.toFixed(1)} %` : "—");
  const memoryText = $derived(isLive ? formatMemory(stats.latest!.memoryBytes) : "—");
  const uptimeText = $derived(isLive ? formatUptime(stats.latest!.uptimeSeconds) : "—");
  const createdText = $derived(formatDateTime(server.createdAtUnix));
</script>

<div class="dash">
  <!-- Proxies get this too: the proxy is the address players actually connect
       to, so it's the one that most needs forwarding. -->
  <button class="address" onclick={() => copy(lanAddress)} title="Click to copy">
    <span class="address-label"><Link2 size={12} /> LAN address — click to copy</span>
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
        <span class="forward-label"><Globe size={12} /> Play over the internet</span>
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
          <span class="address-label"><Link2 size={12} /> Public address — click to copy</span>
          <span class="address-value">{forward.publicAddress}</span>
        </button>
      {/if}
    {/if}
  </div>

  <div class="grid">
    <StatTile label="Status" value={statusMeta.label} />
    <StatTile
      label="Players online"
      value={String(players.length)}
      sub={players.length > 0 ? players.slice(0, 5).join(", ") : "nobody here yet"}
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
    display: flex;
    align-items: center;
    gap: 0.35em;
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
    display: flex;
    align-items: center;
    gap: 0.35em;
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
