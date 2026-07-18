<script lang="ts">
  import { api, type ServerConfig } from "../../api";
  import AddonManager from "../../components/AddonManager.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let curseforgeApiKey = $state<string | null>(null);

  $effect(() => {
    api.getCurseforgeApiKey().then((key) => (curseforgeApiKey = key));
  });

  function sourceBlocked(source: string): string | null {
    if (source === "curseforge" && !curseforgeApiKey) {
      return "Add a CurseForge API key in Settings to browse CurseForge mods.";
    }
    return null;
  }
</script>

<p class="hint">Powered by Modrinth and CurseForge — filtered to {server.loader} and {server.mcVersion}.</p>

<AddonManager
  serverId={server.id}
  kind="mod"
  sources={[
    { value: "modrinth", label: "Modrinth" },
    { value: "curseforge", label: "CurseForge" },
  ]}
  {sourceBlocked}
  list={api.listMods}
  setEnabled={api.setModEnabled}
  remove={api.deleteMod}
  search={api.searchMods}
  install={api.installMod}
  checkUpdates={api.checkModUpdates}
  update={api.updateMod}
/>

<style>
  .hint {
    margin: 0 0 1rem;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
