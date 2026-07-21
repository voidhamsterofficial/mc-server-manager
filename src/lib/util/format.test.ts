import { describe, expect, it } from "vitest";
import { formatFileSize, formatMemory, formatUptime } from "./format";

describe("formatFileSize", () => {
  it("shows plain bytes below a kilobyte", () => {
    expect(formatFileSize(0)).toBe("0 B");
    expect(formatFileSize(512)).toBe("512 B");
    expect(formatFileSize(999)).toBe("999 B");
  });

  it("shows whole KB up to a megabyte — the size most config files are", () => {
    expect(formatFileSize(1000)).toBe("1 KB");
    expect(formatFileSize(4096)).toBe("4 KB");
    expect(formatFileSize(999_000)).toBe("999 KB");
  });

  it("steps up to MB at 1000 KB, not 1024", () => {
    expect(formatFileSize(1_000_000)).toBe("1.0 MB");
    expect(formatFileSize(2_500_000)).toBe("2.5 MB");
  });

  it("switches to GB with two decimals at a gigabyte", () => {
    expect(formatFileSize(1_000_000_000)).toBe("1.00 GB");
    expect(formatFileSize(1_500_000_000)).toBe("1.50 GB");
  });
});

describe("formatMemory", () => {
  it("stays binary so it matches the MB a server was configured with", () => {
    expect(formatMemory(0)).toBe("0 MB");
    expect(formatMemory(1024 * 1024)).toBe("1 MB");
    expect(formatMemory(1536 * 1024)).toBe("2 MB"); // rounds 1.5 -> 2
  });

  it("switches to GB with two decimals at/above a gigabyte", () => {
    expect(formatMemory(1024 * 1024 * 1024)).toBe("1.00 GB");
    expect(formatMemory(1.5 * 1024 * 1024 * 1024)).toBe("1.50 GB");
  });

  it("reports a 2048 MB server as 2.00 GB", () => {
    expect(formatMemory(2048 * 1024 * 1024)).toBe("2.00 GB");
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
