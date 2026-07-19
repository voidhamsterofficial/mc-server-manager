import { describe, expect, it } from "vitest";
import { decodeMotdProperty, encodeMotdProperty } from "./motd";

describe("motd property encoding", () => {
  it("decodes stored § escapes and literal newlines", () => {
    expect(decodeMotdProperty("\\u00A7cHello\\nWorld")).toBe("§cHello\nWorld");
  });

  it("decodes the lowercase \\u00a7 escape too", () => {
    expect(decodeMotdProperty("\\u00a7aGreen")).toBe("§aGreen");
  });

  it("encodes § and newlines to the server.properties form", () => {
    expect(encodeMotdProperty("§cHello\nWorld")).toBe("\\u00A7cHello\\nWorld");
  });

  it("round-trips editor -> stored -> editor", () => {
    const editor = "§aLine one\n§cLine two";
    expect(decodeMotdProperty(encodeMotdProperty(editor))).toBe(editor);
  });
});
