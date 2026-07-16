// Shared presentation metadata for server statuses (single source of truth
// for status colors, labels, and mood).

import type { ServerStatus } from "./api";

export interface StatusMeta {
  label: string;
  emoji: string;
  colorVar: string;
  softVar: string;
  pulsing: boolean;
}

export const STATUS_META: Record<ServerStatus, StatusMeta> = {
  starting: {
    label: "Starting",
    emoji: "🐣",
    colorVar: "--peach",
    softVar: "--peach-soft",
    pulsing: true,
  },
  running: {
    label: "Running",
    emoji: "✨",
    colorVar: "--mint",
    softVar: "--mint-soft",
    pulsing: false,
  },
  stopping: {
    label: "Stopping",
    emoji: "🌙",
    colorVar: "--peach",
    softVar: "--peach-soft",
    pulsing: true,
  },
  stopped: {
    label: "Stopped",
    emoji: "💤",
    colorVar: "--lavender",
    softVar: "--lavender-soft",
    pulsing: false,
  },
  crashed: {
    label: "Crashed",
    emoji: "💥",
    colorVar: "--strawberry",
    softVar: "--strawberry-soft",
    pulsing: false,
  },
};
