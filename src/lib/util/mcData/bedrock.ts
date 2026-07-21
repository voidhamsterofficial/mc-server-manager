// Bedrock Dedicated Server's command set — names only.
//
// Bedrock ships no machine-readable command grammar the way Java's data
// reports do, so this is a name list rather than a tree: it completes command
// names and stops there, instead of inventing argument shapes it can't verify.
//
// Sourced from the wiki's Bedrock command table (commands marked available in
// BE, minus the Education-only and hidden websocket/NPC ones). One list covers
// every BDS version — Bedrock's command set moves slowly enough that splitting
// it per version would be more bookkeeping than accuracy.

import type { CommandNode, McCommandData } from "./types";

const COMMAND_NAMES = [
  "ability", "aimassist", "allowlist", "alwaysday", "camera", "camerashake",
  "changesetting", "clear", "clearspawnpoint", "clone", "connect", "controlscheme",
  "damage", "daylock", "dedicatedwsserver", "deop", "dialogue", "difficulty", "effect",
  "enchant", "event", "execute", "fill", "fog", "function", "gamemode", "gamerule",
  "gametest", "gametips", "give", "help", "hud", "immutableworld", "inputpermission",
  "kick", "kill", "list", "locate", "loot", "me", "mobevent", "msg", "music", "op",
  "ops", "particle", "permission", "place", "playanimation", "playsound", "project",
  "recipe", "reload", "reloadconfig", "reloadpacketlimitconfig", "replaceitem", "ride",
  "save", "say", "schedule", "scoreboard", "script", "scriptevent", "sendshowstoreoffer",
  "set_movement_authority", "setblock", "setmaxplayers", "setworldspawn", "spawnpoint",
  "spreadplayers", "stop", "stopsound", "structure", "summon", "tag", "teleport", "tell",
  "tellraw", "testfor", "testforblock", "testforblocks", "tickingarea", "time", "title",
  "titleraw", "toggledownfall", "tp", "transfer", "w", "wb", "weather", "whitelist",
  "worldbuilder", "wsserver", "xp",
];

function namesOnlyTree(names: string[]): Record<string, CommandNode> {
  const tree: Record<string, CommandNode> = {};
  for (const name of names) {
    tree[name] = {};
  }
  return tree;
}

export const COMMAND_DATA: McCommandData = {
  version: "Bedrock",
  tree: namesOnlyTree(COMMAND_NAMES),
  items: [],
};
