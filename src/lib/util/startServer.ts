// Starting a server, with the port clash caught before anything launches.
//
// Two servers on one port is a common mistake with a terrible failure mode:
// the second one starts, then dies inside the JVM with a bind error that
// reads like a crash. The backend refuses the start either way; this asks
// first, so the answer can be "pick another port" instead of an error toast.

import { api, PROXY_LOADERS, type PortConflict, type ServerConfig } from "../ipc/api";
import { confirmStore } from "../stores/confirm.svelte";
import { serverAddressStore } from "../stores/serverAddress.svelte";
import { textPromptStore } from "../stores/textPrompt.svelte";

const SERVER_PORT_PROPERTY = "server-port";
const LOWEST_ALLOWED_PORT = 1024;
const HIGHEST_ALLOWED_PORT = 65535;

/** The next port up, as the prefilled suggestion. */
function suggestFreePort(takenPort: string): string {
  const taken = Number(takenPort);
  if (!Number.isInteger(taken) || taken >= HIGHEST_ALLOWED_PORT) {
    return "";
  }
  const suggestion = String(taken + 1);
  return suggestion;
}

function parsePort(text: string): number | null {
  const port = Number(text.trim());
  const isValid =
    Number.isInteger(port) && port >= LOWEST_ALLOWED_PORT && port <= HIGHEST_ALLOWED_PORT;
  if (!isValid) {
    return null;
  }
  return port;
}

/**
 * Stops the server holding the port and starts this one instead — after
 * asking again, because anyone playing on the other server is about to be
 * disconnected.
 */
async function stopOtherThenStart(
  conflict: PortConflict,
  server: ServerConfig,
): Promise<void> {
  const confirmed = await confirmStore.ask({
    title: `Stop "${conflict.serverName}"?`,
    body:
      `"${conflict.serverName}" will be shut down so "${server.name}" can take port ` +
      `${conflict.port}. Anyone playing on it right now will be disconnected.\n\n` +
      `Its world is saved as it stops, and you can start it again whenever you like.`,
    confirmLabel: `Stop it & start "${server.name}"`,
    variant: "danger",
  });

  if (confirmed !== "confirm") {
    return;
  }

  await api.stopOtherAndStart(conflict.serverId, server.id);
}

/**
 * Starts a server. If a running one already holds its port, offers to change
 * this server's port first and starts it once changed.
 *
 * Resolves without starting when the user cancels the prompt. Throws for a
 * genuine failure, so callers keep reporting errors the way they already do.
 */
export async function startServerWithPortCheck(server: ServerConfig): Promise<void> {
  const conflict = await api.portConflict(server.id);
  if (conflict === null) {
    await api.startServer(server.id);
    return;
  }

  // Proxies keep their listen port in their own config file (velocity.toml,
  // BungeeCord's config.yml), which this doesn't know how to rewrite — so
  // they're offered the other way out rather than a port box that would edit
  // the wrong file. Stopping the other server works the same either way.
  const canChangePort = !PROXY_LOADERS.includes(server.loader);
  const portAdvice = canChangePort
    ? `Stop "${conflict.serverName}" and start this one in its place, or give "${server.name}" a port of its own and leave "${conflict.serverName}" running.`
    : `Stop "${conflict.serverName}" and start this one in its place, or change this proxy's listen port in its own config file and try again.`;

  // Explained before anything happens. Neither way out is trivial — stopping
  // a server disconnects whoever is on it, and changing a port changes the
  // address everyone connects to — so the clash is described first, and both
  // routes ask again before acting.
  const choice = await confirmStore.ask({
    title: `Port ${conflict.port} is already in use`,
    body:
      `"${conflict.serverName}" is running and listening on port ${conflict.port}, and two ` +
      `servers can't share one — so "${server.name}" can't start right now.\n\n` +
      portAdvice,
    confirmLabel: `Stop "${conflict.serverName}" & start this`,
    variant: "primary",
    secondaryLabel: canChangePort ? "Change this server's port…" : undefined,
    secondaryVariant: "danger",
  });

  if (choice === "cancel") {
    return;
  }

  if (choice === "confirm") {
    await stopOtherThenStart(conflict, server);
    return;
  }

  const chosen = await textPromptStore.ask({
    title: `New port for "${server.name}"`,
    hint: `Anything between ${LOWEST_ALLOWED_PORT} and ${HIGHEST_ALLOWED_PORT} that no other server is using. The default for Minecraft is 25565.`,
    actionLabel: "Change port & start",
    variant: "primary",
    placeholder: "25566",
    initialValue: suggestFreePort(conflict.port),
    required: true,
  });

  if (chosen === null) {
    return;
  }

  const port = parsePort(chosen);
  if (port === null) {
    throw new Error(
      `"${chosen}" isn't a usable port — pick a number between ${LOWEST_ALLOWED_PORT} and ${HIGHEST_ALLOWED_PORT}.`,
    );
  }

  await api.saveServerProperties(server.id, [
    { key: SERVER_PORT_PROPERTY, value: String(port) },
  ]);
  // The dashboard shows the connect address, which just changed.
  serverAddressStore.markChanged(server.id);
  await api.startServer(server.id);
}
