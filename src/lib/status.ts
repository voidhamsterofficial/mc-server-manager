// Shared presentation metadata for server statuses (single source of truth
// for status colors, labels, and mood).

import { Rocket, Sparkles, Hourglass, Moon, Bomb } from "@lucide/svelte";
import type { Component } from "svelte";
import type { ServerStatus } from "./api";

export interface StatusMeta {
  label: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  icon: Component<any>;
  colorVar: string;
  softVar: string;
  pulsing: boolean;
}

export const STATUS_META: Record<ServerStatus, StatusMeta> = {
  starting: {
    label: "Starting",
    icon: Rocket,
    colorVar: "--peach",
    softVar: "--peach-soft",
    pulsing: true,
  },
  running: {
    label: "Running",
    icon: Sparkles,
    colorVar: "--mint",
    softVar: "--mint-soft",
    pulsing: false,
  },
  stopping: {
    label: "Stopping",
    icon: Hourglass,
    colorVar: "--peach",
    softVar: "--peach-soft",
    pulsing: true,
  },
  stopped: {
    label: "Stopped",
    icon: Moon,
    colorVar: "--lavender",
    softVar: "--lavender-soft",
    pulsing: false,
  },
  crashed: {
    label: "Crashed",
    icon: Bomb,
    colorVar: "--strawberry",
    softVar: "--strawberry-soft",
    pulsing: false,
  },
};
