// Autocomplete for the console's command box.
//
// The command set is not written by hand and not global: each Minecraft
// version has its own dataset under ./mcData, carrying the real Brigadier
// tree — literal subcommands, argument names, and argument types. Callers
// load the dataset for the server they're looking at and pass it in; a server
// on a version with no dataset gets `null` and no suggestions.
//
// Completion works by walking that tree alongside the tokens already typed,
// then offering whatever the node the caret has reached will accept.

import type { ArgKind, CommandNode, McCommandData } from "./mcData";

/** Values for argument kinds with a small fixed set. Item ids come from the
 *  dataset; player names come from whoever is online. */
const FIXED_CHOICES: Partial<Record<ArgKind, string[]>> = {
  gamemode: ["survival", "creative", "adventure", "spectator"],
  dimension: ["overworld", "the_nether", "the_end"],
  bool: ["true", "false"],
};

const PLAYER_SELECTORS = ["@a", "@p", "@r", "@s"];
const ENTITY_SELECTORS = [...PLAYER_SELECTORS, "@e"];

/** Cap on how many completions are handed back, so typing `give x i` doesn't
 *  try to render a thousand item rows. */
const MAX_SUGGESTIONS = 40;

export interface Suggestion {
  /** The token text inserted when accepted. */
  value: string;
  /** Right-hand hint: what this choice is. */
  detail: string;
}

/** Which token the caret is in, and what has been typed of it so far. */
interface TokenContext {
  index: number;
  partial: string;
}

function tokenContext(input: string): TokenContext {
  // A trailing space means the previous token is finished and the caret has
  // moved on to a fresh, empty one.
  const isStartingNewToken = input.endsWith(" ");
  const tokens = input.split(/\s+/).filter((token) => token !== "");
  if (isStartingNewToken) {
    return { index: tokens.length, partial: "" };
  }
  return {
    index: Math.max(0, tokens.length - 1),
    partial: tokens[tokens.length - 1] ?? "",
  };
}

function matchesPartial(candidate: string, partial: string): boolean {
  return candidate.toLowerCase().startsWith(partial.toLowerCase());
}

/** Follows `path` from a node, resolving redirects. Returns null when the
 *  tokens don't correspond to anything the grammar accepts. */
function resolveNode(tree: Record<string, CommandNode>, path: string[]): CommandNode | null {
  const rootNode: CommandNode = { c: tree };
  let current: CommandNode = rootNode;

  for (const token of path) {
    let children = current.c;

    // A redirect hands parsing back to another subtree — `execute as @a run`
    // restarts at `execute`, so the tokens after it are matched there.
    if (children === undefined && current.r !== undefined) {
      const target = resolveNode(tree, current.r);
      if (target === null) {
        return null;
      }
      children = target.c;
    }
    if (children === undefined) {
      return null;
    }

    // `Object.hasOwn`, not a bare lookup: a token like `toString` would
    // otherwise find Object.prototype's method and be walked into as if it
    // were a command node.
    const literal = Object.hasOwn(children, token) ? children[token] : undefined;
    if (literal !== undefined && literal.k === undefined) {
      current = literal;
      continue;
    }

    // Not a literal, so it must be filling an argument slot. Any argument
    // child will do — the tree never offers a literal and an argument that
    // could both be meant, beyond the literal we just checked for.
    const argumentChild = Object.values(children).find((child) => child.k !== undefined);
    if (argumentChild === undefined) {
      return null;
    }
    current = argumentChild;
  }

  return current;
}

function choicesForKind(
  kind: ArgKind,
  partial: string,
  onlinePlayers: string[],
  items: string[],
): Suggestion[] {
  if (kind === "player" || kind === "entity") {
    const matching = onlinePlayers.filter((player) => matchesPartial(player, partial));
    const players: Suggestion[] = matching.map((player) => ({
      value: player,
      detail: "online player",
    }));
    // Selectors are valid wherever a target is. `@e` is only offered for a
    // general entity argument — it can match mobs, which a player-restricted
    // argument would reject.
    const allSelectors = kind === "entity" ? ENTITY_SELECTORS : PLAYER_SELECTORS;
    const selectors = allSelectors.filter((selector) => matchesPartial(selector, partial));
    return [...players, ...selectors.map((value) => ({ value, detail: "selector" }))];
  }

  if (kind === "item") {
    // Substring, not prefix: nobody remembers whether it's `diamond_sword` or
    // `sword_diamond`, and "sword" should find both.
    const needle = partial.toLowerCase();
    const matching = items.filter((id) => id.includes(needle));
    return matching.map((id) => ({ value: id, detail: "item" }));
  }

  const fixed = FIXED_CHOICES[kind];
  if (fixed === undefined) {
    return [];
  }
  const matching = fixed.filter((choice) => matchesPartial(choice, partial));
  return matching.map((choice) => ({ value: choice, detail: kind }));
}

/**
 * Completions for whatever is currently being typed: command names in the
 * first position, then whatever the command's grammar accepts at the position
 * the caret has reached. Empty when there's nothing useful to offer — an
 * unrecognised command, or a free-form argument like coordinates or NBT.
 */
export function suggestCompletions(
  input: string,
  data: McCommandData | null,
  onlinePlayers: string[],
): Suggestion[] {
  if (data === null) {
    return [];
  }
  if (input.trim() === "" && !input.endsWith(" ")) {
    return [];
  }

  const context = tokenContext(input);
  const tokens = input.split(/\s+/).filter((token) => token !== "");
  const node = resolveNode(data.tree, tokens.slice(0, context.index));
  if (node === null) {
    return [];
  }

  let children = node.c;
  if (children === undefined && node.r !== undefined) {
    children = resolveNode(data.tree, node.r)?.c;
  }
  if (children === undefined) {
    return [];
  }

  const suggestions: Suggestion[] = [];
  for (const [name, child] of Object.entries(children)) {
    if (child.k === undefined) {
      if (matchesPartial(name, context.partial)) {
        suggestions.push({ value: name, detail: context.index === 0 ? "command" : "subcommand" });
      }
      continue;
    }
    suggestions.push(...choicesForKind(child.k, context.partial, onlinePlayers, data.items));
  }

  return suggestions.slice(0, MAX_SUGGESTIONS);
}

/** The input with its in-progress token replaced by `value`, ready for the
 *  next argument. */
export function applyCompletion(input: string, value: string): string {
  const { index } = tokenContext(input);
  const tokens = input.split(/\s+/).filter((token) => token !== "");
  const kept = tokens.slice(0, index);
  return [...kept, value].join(" ") + " ";
}

/** Argument names the command being typed still expects, as a usage hint —
 *  built from the tree so it can't disagree with what's completed. */
export function usageHint(input: string, data: McCommandData | null): string | null {
  if (data === null) {
    return null;
  }
  const commandName = input.trim().split(/\s+/)[0] ?? "";
  if (commandName === "" || !Object.hasOwn(data.tree, commandName)) {
    return null;
  }

  const tokens = input.split(/\s+/).filter((token) => token !== "");
  const { index } = tokenContext(input);
  const node = resolveNode(data.tree, tokens.slice(0, index));
  if (node === null || node.c === undefined) {
    return null;
  }

  const names = Object.entries(node.c).map(([name, child]) => {
    if (child.k === undefined) {
      return name;
    }
    return `<${name}>`;
  });
  if (names.length === 0) {
    return null;
  }
  return `${commandName}: ${names.slice(0, 8).join(" | ")}`;
}

/** Whether a command's first argument takes a player, for the quick-command
 *  menu's player submenus. */
export function takesPlayerFirst(name: string, data: McCommandData | null): boolean {
  if (data === null) {
    return false;
  }
  if (!Object.hasOwn(data.tree, name)) {
    return false;
  }
  const command = data.tree[name];
  if (command?.c === undefined) {
    return false;
  }
  return Object.values(command.c).some((child) => child.k === "player");
}
