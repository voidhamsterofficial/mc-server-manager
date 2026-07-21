import { describe, expect, it } from "vitest";
import { EMPTY_FILTER, filterConsoleLines, isFilterActive, lineText } from "./consoleFilter";
import type { ConsoleLine, LogLevel } from "../ipc/events";

function line(text: string, level: LogLevel = "info"): ConsoleLine {
  return { spans: [{ text, bold: false }], level };
}

const LINES: ConsoleLine[] = [
  line("Starting minecraft server version 1.21.1"),
  line("Steve joined the game"),
  line("Can't keep up! Is the server overloaded?", "warn"),
  line("steve issued server command: /home"),
  line("Exception in thread main", "error"),
];

function texts(lines: ConsoleLine[]): string[] {
  return lines.map(lineText);
}

describe("filterConsoleLines", () => {
  it("returns everything when nothing is filtered", () => {
    expect(filterConsoleLines(LINES, EMPTY_FILTER)).toHaveLength(5);
  });

  it("matches text case-insensitively, anywhere in the line", () => {
    const found = filterConsoleLines(LINES, { query: "STEVE", level: "all" });
    expect(texts(found)).toEqual(["Steve joined the game", "steve issued server command: /home"]);
  });

  it("ignores surrounding whitespace in the query", () => {
    expect(filterConsoleLines(LINES, { query: "  joined  ", level: "all" })).toHaveLength(1);
  });

  it("filters to errors only", () => {
    const found = filterConsoleLines(LINES, { query: "", level: "error" });
    expect(texts(found)).toEqual(["Exception in thread main"]);
  });

  it("includes errors when filtering to warnings, since they matter more", () => {
    const found = filterConsoleLines(LINES, { query: "", level: "warn" });
    expect(texts(found)).toEqual([
      "Can't keep up! Is the server overloaded?",
      "Exception in thread main",
    ]);
  });

  it("combines text and level", () => {
    expect(filterConsoleLines(LINES, { query: "server", level: "warn" })).toHaveLength(1);
    expect(filterConsoleLines(LINES, { query: "steve", level: "error" })).toHaveLength(0);
  });

  it("preserves the original order", () => {
    const found = filterConsoleLines(LINES, { query: "e", level: "all" });
    expect(texts(found)).toEqual(texts(LINES.filter((l) => lineText(l).includes("e"))));
  });
});

describe("isFilterActive", () => {
  it("is false only when nothing would be hidden", () => {
    expect(isFilterActive(EMPTY_FILTER)).toBe(false);
    expect(isFilterActive({ query: "   ", level: "all" })).toBe(false);
    expect(isFilterActive({ query: "x", level: "all" })).toBe(true);
    expect(isFilterActive({ query: "", level: "error" })).toBe(true);
  });
});
