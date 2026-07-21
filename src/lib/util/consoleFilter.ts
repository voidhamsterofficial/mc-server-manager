// Filtering for the console view: narrowing 5000 lines down to the ones a
// question is actually about — a player's name, or just the errors.

import type { ConsoleLine, LogLevel } from "../ipc/events";

/** Which severities to show. `all` is the default, and the only setting that
 *  shows lines the parser couldn't classify beyond "info". */
export type LevelFilter = "all" | "warn" | "error";

export interface ConsoleFilter {
  /** Case-insensitive substring; empty matches everything. */
  query: string;
  level: LevelFilter;
}

export const EMPTY_FILTER: ConsoleFilter = { query: "", level: "all" };

/** The line's text with its spans joined — what a search matches against. */
export function lineText(line: ConsoleLine): string {
  return line.spans.map((span) => span.text).join("");
}

function matchesLevel(level: LogLevel, filter: LevelFilter): boolean {
  if (filter === "all") {
    return true;
  }
  if (filter === "error") {
    return level === "error";
  }
  // "warn" means warnings *and* errors — anything worth attention. Showing
  // warnings while hiding the errors they often precede would be worse than
  // useless when someone is chasing a crash.
  return level === "warn" || level === "error";
}

export function isFilterActive(filter: ConsoleFilter): boolean {
  return filter.query.trim() !== "" || filter.level !== "all";
}

/** The lines a filter admits, in their original order. */
export function filterConsoleLines(
  lines: ConsoleLine[],
  filter: ConsoleFilter,
): ConsoleLine[] {
  if (!isFilterActive(filter)) {
    return lines;
  }

  const needle = filter.query.trim().toLowerCase();
  return lines.filter((line) => {
    if (!matchesLevel(line.level, filter.level)) {
      return false;
    }
    if (needle === "") {
      return true;
    }
    return lineText(line).toLowerCase().includes(needle);
  });
}
