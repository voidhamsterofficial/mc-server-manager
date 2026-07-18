import { describe, expect, it } from "vitest";
import { formatBytes, formatUptime } from "./format";

describe("formatBytes", () => {
  it("shows whole MB below a gigabyte", () => {
    expect(formatBytes(0)).toBe("0 MB");
    expect(formatBytes(1024 * 1024)).toBe("1 MB");
    expect(formatBytes(1536 * 1024)).toBe("2 MB"); // rounds 1.5 -> 2
  });

  it("switches to GB with two decimals at/above a gigabyte", () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.00 GB");
    expect(formatBytes(1.5 * 1024 * 1024 * 1024)).toBe("1.50 GB");
  });
});

describe("formatUptime", () => {
  it("uses h/m once there's at least an hour", () => {
    expect(formatUptime(3600)).toBe("1h 0m");
    expect(formatUptime(3661)).toBe("1h 1m");
  });

  it("uses m/s under an hour", () => {
    expect(formatUptime(61)).toBe("1m 1s");
    expect(formatUptime(600)).toBe("10m 0s");
  });

  it("uses just seconds under a minute", () => {
    expect(formatUptime(0)).toBe("0s");
    expect(formatUptime(45)).toBe("45s");
  });
});
