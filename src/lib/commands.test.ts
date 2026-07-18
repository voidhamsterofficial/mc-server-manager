import { describe, expect, it } from "vitest";
import { commandArg, commandText } from "./commands";

describe("commandArg", () => {
  it("passes a plain vanilla name through unchanged", () => {
    expect(commandArg("Notch")).toBe("Notch");
  });

  it("quotes names containing spaces (Bedrock gamertags)", () => {
    expect(commandArg("Cool Player 99")).toBe('"Cool Player 99"');
  });

  it("strips control characters that could inject a second command", () => {
    // A newline would otherwise smuggle in a whole extra console command.
    expect(commandArg("Alex\nop Alex")).toBe('"Alex op Alex"');
    expect(commandArg("Bad\r\nstop")).toBe('"Bad stop"');
    expect(commandArg("tab\tname")).toBe('"tab name"');
  });

  it("trims surrounding whitespace", () => {
    expect(commandArg("  Steve  ")).toBe("Steve");
  });
});

describe("commandText", () => {
  it("keeps multi-word reasons intact", () => {
    expect(commandText("griefing the spawn")).toBe("griefing the spawn");
  });

  it("strips CR/LF so a reason can't inject a command", () => {
    expect(commandText("spam\nstop")).toBe("spam stop");
  });
});
