import { describe, expect, it } from "vitest";
import { resolveBackupsDir, supportsPlugins, type ServerConfig } from "./api";

describe("resolveBackupsDir", () => {
  it("returns the explicit backups dir when one is set", () => {
    const server = { dir: "/srv/mc", backupsDir: "/elsewhere/backups" } as ServerConfig;
    expect(resolveBackupsDir(server)).toBe("/elsewhere/backups");
  });

  it("appends /backups next to the server on unix paths", () => {
    const server = { dir: "/srv/mc", backupsDir: null } as ServerConfig;
    expect(resolveBackupsDir(server)).toBe("/srv/mc/backups");
  });

  it("uses a backslash for windows paths", () => {
    const server = { dir: "C:\\servers\\mc", backupsDir: null } as ServerConfig;
    expect(resolveBackupsDir(server)).toBe("C:\\servers\\mc\\backups");
  });
});

describe("supportsPlugins", () => {
  it("is true for the Paper family", () => {
    expect(supportsPlugins("paper")).toBe(true);
  });

  it("is false for vanilla and mod loaders", () => {
    expect(supportsPlugins("vanilla")).toBe(false);
    expect(supportsPlugins("fabric")).toBe(false);
  });
});
