<script lang="ts">
  import { fade } from "svelte/transition";
  import { api, type ScheduledTask, type ServerConfig, type TaskAction } from "../../api";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatDateTime } from "../../format";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  const CRON_PRESETS = [
    { label: "Every 15 minutes", cron: "*/15 * * * *" },
    { label: "Every hour", cron: "0 * * * *" },
    { label: "Every 6 hours", cron: "0 */6 * * *" },
    { label: "Daily at 4:00", cron: "0 4 * * *" },
    { label: "Custom cron…", cron: "custom" },
  ];

  const ACTION_KINDS = [
    { value: "command", label: "💬 Run command" },
    { value: "backup", label: "🎁 Back up" },
    { value: "restart", label: "🔄 Restart" },
    { value: "start", label: "▶ Start" },
    { value: "stop", label: "⏹ Stop" },
  ] as const;

  let tasks = $state<ScheduledTask[]>([]);
  let editing = $state(false);
  let editingId = $state("");
  let formName = $state("");
  let formPreset = $state(CRON_PRESETS[1].cron);
  let formCustomCron = $state("0 * * * *");
  let formActionKind = $state<TaskAction["type"]>("command");
  let formCommand = $state("");
  let formEnabled = $state(true);
  let nextRunText = $state("");
  let saving = $state(false);

  const myTasks = $derived(tasks.filter((task) => task.serverId === server.id));
  const effectiveCron = $derived(formPreset === "custom" ? formCustomCron : formPreset);

  $effect(() => {
    loadTasks();
  });

  $effect(() => {
    if (!editing) {
      return;
    }
    previewNextRun(effectiveCron);
  });

  async function loadTasks() {
    try {
      tasks = await api.listTasks();
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function previewNextRun(cron: string) {
    try {
      const nextUnix = await api.previewNextRun(cron);
      nextRunText = nextUnix === null ? "never" : formatDateTime(nextUnix);
    } catch {
      nextRunText = "invalid cron expression";
    }
  }

  function startCreate() {
    editingId = "";
    formName = "";
    formPreset = CRON_PRESETS[1].cron;
    formCustomCron = "0 * * * *";
    formActionKind = "command";
    formCommand = "";
    formEnabled = true;
    editing = true;
  }

  function startEdit(task: ScheduledTask) {
    editingId = task.id;
    formName = task.name;
    const matchingPreset = CRON_PRESETS.find((preset) => preset.cron === task.cron);
    formPreset = matchingPreset ? matchingPreset.cron : "custom";
    formCustomCron = task.cron;
    formActionKind = task.action.type;
    formCommand = task.action.type === "command" ? task.action.command : "";
    formEnabled = task.enabled;
    editing = true;
  }

  function buildAction(): TaskAction {
    if (formActionKind === "command") {
      return { type: "command", command: formCommand.trim() };
    }
    return { type: formActionKind };
  }

  async function saveTask(event: SubmitEvent) {
    event.preventDefault();
    if (formActionKind === "command" && formCommand.trim() === "") {
      toastsStore.show("The task needs a command to run ✍️");
      return;
    }
    saving = true;
    try {
      await api.upsertTask({
        id: editingId,
        serverId: server.id,
        name: formName,
        cron: effectiveCron,
        action: buildAction(),
        enabled: formEnabled,
      });
      toastsStore.success("Task scheduled ⏰");
      editing = false;
      await loadTasks();
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      saving = false;
    }
  }

  async function toggleTask(task: ScheduledTask) {
    try {
      await api.upsertTask({ ...task, enabled: !task.enabled });
      await loadTasks();
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function removeTask(taskId: string) {
    try {
      await api.deleteTask(taskId);
      await loadTasks();
      toastsStore.show("Task removed");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function runNow(taskId: string) {
    try {
      await api.runTaskNow(taskId);
      toastsStore.success("Task fired 🚀");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  function describeAction(action: TaskAction): string {
    if (action.type === "command") {
      return `💬 ${action.command}`;
    }
    const found = ACTION_KINDS.find((kind) => kind.value === action.type);
    return found?.label ?? action.type;
  }
</script>

<div class="scheduler-tab">
  <div class="head">
    <p class="hint">Automate restarts, backups, and commands — no plugins needed.</p>
    {#if !editing}
      <Button onclick={startCreate}>＋ New task</Button>
    {/if}
  </div>

  {#if editing}
    <form class="editor" onsubmit={saveTask} in:fade={{ duration: 120 }}>
      <div class="editor-grid">
        <label>
          <span>Name</span>
          <input type="text" bind:value={formName} placeholder="Nightly backup" maxlength="48" />
        </label>
        <label>
          <span>Runs</span>
          <select bind:value={formPreset}>
            {#each CRON_PRESETS as preset (preset.cron)}
              <option value={preset.cron}>{preset.label}</option>
            {/each}
          </select>
        </label>
        {#if formPreset === "custom"}
          <label>
            <span>Cron expression</span>
            <input type="text" bind:value={formCustomCron} spellcheck="false" />
          </label>
        {/if}
        <label>
          <span>Action</span>
          <select bind:value={formActionKind}>
            {#each ACTION_KINDS as kind (kind.value)}
              <option value={kind.value}>{kind.label}</option>
            {/each}
          </select>
        </label>
        {#if formActionKind === "command"}
          <label class="wide">
            <span>Command</span>
            <input
              type="text"
              bind:value={formCommand}
              placeholder="say Backup starting soon!"
              spellcheck="false"
            />
          </label>
        {/if}
      </div>
      <div class="editor-footer">
        <label class="toggle">
          <input type="checkbox" bind:checked={formEnabled} />
          <span>Enabled</span>
        </label>
        <span class="next-run">Next run: {nextRunText}</span>
        <span class="editor-buttons">
          <Button variant="ghost" onclick={() => (editing = false)}>Cancel</Button>
          <Button type="submit" disabled={saving || formName.trim() === ""}>Save task</Button>
        </span>
      </div>
    </form>
  {/if}

  {#if myTasks.length === 0 && !editing}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="face">🕰️</span>
      <p>Nothing scheduled — add a nightly backup or a friendly hourly broadcast!</p>
    </div>
  {:else}
    <ul class="task-list">
      {#each myTasks as task (task.id)}
        <li class:disabled={!task.enabled} in:fade={{ duration: 120 }}>
          <label class="toggle" title={task.enabled ? "Disable" : "Enable"}>
            <input type="checkbox" checked={task.enabled} onchange={() => toggleTask(task)} />
          </label>
          <span class="task-info">
            <span class="task-name">{task.name}</span>
            <span class="task-meta">{describeAction(task.action)} · <code>{task.cron}</code></span>
          </span>
          <span class="row-actions">
            <Button variant="soft" onclick={() => runNow(task.id)}>Run now</Button>
            <Button variant="ghost" onclick={() => startEdit(task)}>✏️</Button>
            <Button variant="ghost" onclick={() => removeTask(task.id)}>🗑</Button>
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .scheduler-tab {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 1rem;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .editor {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1rem 1.25rem;
  }

  .editor-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.75rem;
  }

  .wide {
    grid-column: 1 / -1;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  input[type="text"],
  select {
    font-family: inherit;
    font-size: 0.95rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.5em 0.8em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  input[type="text"]:focus,
  select:focus {
    border-color: var(--accent);
  }

  .editor-footer {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 0.9rem;
    flex-wrap: wrap;
  }

  .toggle {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
  }

  .toggle input {
    width: 1.15rem;
    height: 1.15rem;
    accent-color: var(--accent);
  }

  .next-run {
    flex: 1;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .editor-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 2.5rem 0;
  }

  .face {
    font-size: 2.6rem;
    display: inline-block;
    animation: tick 2s ease-in-out infinite;
  }

  @keyframes tick {
    0%,
    100% {
      transform: rotate(-6deg);
    }
    50% {
      transform: rotate(6deg);
    }
  }

  .task-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .task-list li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.6rem 0.9rem;
    transition: opacity 0.2s ease;
  }

  .task-list li.disabled {
    opacity: 0.55;
  }

  .task-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .task-name {
    font-weight: 700;
  }

  .task-meta {
    font-size: 0.8rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-meta code {
    font-family: var(--font-mono);
    background: var(--surface-2);
    border-radius: 6px;
    padding: 0.05em 0.4em;
  }

  .row-actions {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }
</style>
