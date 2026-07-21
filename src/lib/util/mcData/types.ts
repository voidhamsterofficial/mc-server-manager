// Shapes shared by every version's command data.

/** How an argument can be completed. `any` means free-form (coordinates, NBT,
 *  arbitrary text) and offers nothing. */
export type ArgKind =
  | "player"
  | "entity"
  | "item"
  | "gamemode"
  | "dimension"
  | "number"
  | "bool"
  | "message"
  | "any";

export interface CommandNode {
  /** Child nodes by name: a literal subcommand, or an argument's own name. */
  c?: Record<string, CommandNode>;
  /** Present on argument nodes only. */
  k?: ArgKind;
  /** Path this node hands parsing back to, e.g. ["execute"]. */
  r?: string[];
}

/** One Minecraft version's command set. */
export interface McCommandData {
  /** The version this was captured from, for display. */
  version: string;
  /** Command tree, keyed by command name. A command with no children still
   *  completes its own name — that's all a names-only dataset knows. */
  tree: Record<string, CommandNode>;
  /** Item ids without the `minecraft:` namespace. Empty when unknown. */
  items: string[];
}
