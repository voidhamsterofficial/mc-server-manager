<script lang="ts">
  import { api, type ServerConfig } from "../../ipc/api";
  import { FEATURE_COLOR } from "../../util/features";
  import AddonManager from "../../components/AddonManager.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();
</script>

<p class="hint">Powered by Modrinth and SpigotMC — filtered to {server.loader} and {server.mcVersion}.</p>

<AddonManager
  serverId={server.id}
  kind="plugin"
  accentColor={FEATURE_COLOR.plugins}
  sources={[
    { value: "modrinth", label: "Modrinth" },
    { value: "spigot", label: "SpigotMC" },
  ]}
  list={api.listPlugins}
  setEnabled={api.setPluginEnabled}
  remove={api.deletePlugin}
  search={api.searchPlugins}
  install={api.installPlugin}
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
