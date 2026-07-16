<script lang="ts">
  import type { ServerConfig } from "../../api";
  import { serversStore } from "../../stores/servers.svelte";
  import { statsStore } from "../../stores/stats.svelte";
  import { formatBytes, formatUptime } from "../../format";
  import { STATUS_META } from "../../status";
  import StatTile from "../../components/StatTile.svelte";
  import Sparkline from "../../components/Sparkline.svelte";

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

  // Fixed memory scale relative to the configured heap keeps the sparkline
  // stable; the JVM can exceed -Xmx with off-heap memory, hence the headroom.
  const memoryScaleMax = $derived(server.memoryMb * BYTES_PER_MB * 1.5);

  const cpuText = $derived(isLive ? `${stats.latest!.cpuPercent.toFixed(1)} %` : "—");
  const memoryText = $derived(isLive ? formatBytes(stats.latest!.memoryBytes) : "—");
  const uptimeText = $derived(isLive ? formatUptime(stats.latest!.uptimeSeconds) : "—");
</script>

<div class="grid">
  <StatTile label="Status" value="{statusMeta.label} {statusMeta.emoji}" />
  <StatTile
    label="Players online"
    value={String(players.length)}
    sub={players.length > 0 ? players.slice(0, 5).join(", ") : "nobody here yet 🌙"}
  />
  <StatTile label="Uptime" value={uptimeText} />
  <StatTile
    label="CPU"
    value={cpuText}
    sub={isLive ? "of the whole machine" : "start the server to see stats"}
  >
    {#if stats.cpuHistory.length > 1}
      <Sparkline values={stats.cpuHistory} max={100} color="var(--chart-cpu)" />
    {/if}
  </StatTile>
  <StatTile
    label="Memory"
    value={memoryText}
    sub="allocated: {server.memoryMb} MB"
  >
    {#if stats.memoryHistory.length > 1}
      <Sparkline values={stats.memoryHistory} max={memoryScaleMax} color="var(--chart-mem)" />
    {/if}
  </StatTile>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 1.2rem;
    padding: 0.25rem 0.25rem 1rem;
  }
</style>
