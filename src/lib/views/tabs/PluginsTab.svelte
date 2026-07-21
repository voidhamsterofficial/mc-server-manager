<script lang="ts">
  import { api, type ServerConfig } from "../../ipc/api";
  import { FEATURE_COLOR } from "../../util/features";
  import AddonManager from "../../components/AddonManager.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();
</script>

<p class="hint">Powered by Modrinth — filtered to {server.loader} and {server.mcVersion}.</p>

<!--
  SpigotMC is hidden, not deleted. SpigotMC publishes no API of its own, so
  browsing it means going through api.spiget.org — a third-party community
  mirror with no auth, no key we can register for, and no rate limit we can
  raise. In practice its requests fail often enough that the source isn't
  dependable enough to offer, and there is nothing on our side that would fix
  it. The backend (`addons::sources::spigot`) and the "spigot" AddonSource
  variant are left intact: installs already recorded against SpigotMC still
  resolve and update, and re-adding the entry below is all it takes to bring
  the browser back if Spiget becomes reliable or Spigot ships a real API.
-->
<AddonManager
  serverId={server.id}
  kind="plugin"
  accentColor={FEATURE_COLOR.plugins}
  sources={[{ value: "modrinth", label: "Modrinth" }]}
  list={api.listPlugins}
  setEnabled={api.setPluginEnabled}
  remove={api.deletePlugin}
  search={api.searchPlugins}
  install={api.installPlugin}
  importJar={api.importPluginJar}
  checkUpdates={api.checkPluginUpdates}
  update={api.updatePlugin}
/>

<style>
  .hint {
    margin: 0 0 1rem;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
