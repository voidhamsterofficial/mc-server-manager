// Single source of truth for the per-feature icon color. Each area of a
// server (the tab it lives under) has one hue, so its icon reads the same in
// the tab nav, that tab's section headers, and its empty states. Values are
// CSS custom properties defined in theme.css (light + dark variants).

export type FeatureId =
  | "dashboard"
  | "console"
  | "players"
  | "plugins"
  | "mods"
  | "files"
  | "settings"
  | "backups"
  | "scheduler";

export const FEATURE_COLOR: Record<FeatureId, string> = {
  dashboard: "var(--feat-dashboard)",
  console: "var(--feat-console)",
  players: "var(--feat-players)",
  plugins: "var(--feat-plugins)",
  mods: "var(--feat-mods)",
  files: "var(--feat-files)",
  settings: "var(--feat-settings)",
  backups: "var(--feat-backups)",
  scheduler: "var(--feat-scheduler)",
};
