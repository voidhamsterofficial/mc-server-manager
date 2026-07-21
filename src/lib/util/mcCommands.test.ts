import { describe, expect, it } from "vitest";
import { applyCompletion, suggestCompletions, takesPlayerFirst, usageHint } from "./mcCommands";
import { COMMAND_DATA as JAVA_26_2 } from "./mcData/v26_2";
import { COMMAND_DATA as BEDROCK } from "./mcData/bedrock";
import { hasCommandData, loadCommandData } from "./mcData";

const PLAYERS = ["Squ1ggly", "Steve", "sam"];

function values(input: string, players = PLAYERS, data = JAVA_26_2): string[] {
  return suggestCompletions(input, data, players).map((s) => s.value);
}

describe("per-version datasets", () => {
  it("carries the full vanilla command set and item registry", () => {
    expect(Object.keys(JAVA_26_2.tree).length).toBeGreaterThan(80);
    expect(JAVA_26_2.tree).toHaveProperty("execute");
    expect(JAVA_26_2.items).toContain("diamond_sword");
    expect(JAVA_26_2.items.length).toBeGreaterThan(1000);
  });

  it("only claims versions it actually has data for", async () => {
    expect(hasCommandData("26.2")).toBe(true);
    expect(hasCommandData("bedrock")).toBe(true);
    expect(hasCommandData("1.20.1")).toBe(false);
    await expect(loadCommandData("1.20.1")).resolves.toBeNull();
    await expect(loadCommandData("26.2")).resolves.toMatchObject({ version: "26.2" });
  });

  it("never falls back to a near-enough version", () => {
    // 1.21.1 is not 26.2, and guessing would suggest commands the server
    // would reject.
    expect(hasCommandData("1.21")).toBe(false);
    expect(hasCommandData("26.1")).toBe(false);
  });
});

describe("with no dataset for the server's version", () => {
  it("suggests nothing at all", () => {
    expect(suggestCompletions("giv", null, PLAYERS)).toEqual([]);
    expect(suggestCompletions("give Steve ", null, PLAYERS)).toEqual([]);
    expect(usageHint("give ", null)).toBeNull();
    expect(takesPlayerFirst("give", null)).toBe(false);
  });
});

describe("suggestCompletions", () => {
  it("offers nothing for an empty box", () => {
    expect(values("")).toEqual([]);
  });

  it("completes command names by prefix, case-insensitively", () => {
    expect(values("gam")).toEqual(["gamemode", "gamerule"]);
    expect(values("GIV")).toEqual(["give"]);
  });

  it("completes literal subcommands from the real grammar", () => {
    expect(values("execute ")).toContain("run");
    expect(values("execute ")).toContain("as");
    expect(values("scoreboard ")).toEqual(["objectives", "players"]);
    expect(values("data ")).toEqual(["get", "merge", "modify", "remove"]);
  });

  it("offers online players where the grammar wants a player", () => {
    expect(values("give ")).toEqual([...PLAYERS, "@a", "@p", "@r", "@s"]);
    expect(values("give St")).toEqual(["Steve"]);
  });

  it("offers item ids, matched anywhere in the name", () => {
    const suggestions = values("give Steve diamond_s");
    expect(suggestions).toContain("diamond_sword");

    // Substring matching: "sword" finds every sword, not just one starting
    // with it.
    const swords = values("give Steve sword");
    expect(swords).toContain("diamond_sword");
    expect(swords).toContain("netherite_sword");
  });

  it("respects argument position, not just the command", () => {
    // gamemode is <mode> then [target].
    expect(values("gamemode ")).toEqual(["survival", "creative", "adventure", "spectator"]);
    // A player-restricted target: names and player selectors, but not @e,
    // which could match a mob.
    expect(values("gamemode creative ")).toEqual([...PLAYERS, "@a", "@p", "@r", "@s"]);
  });

  it("offers entity selectors where an entity is allowed", () => {
    expect(values("kill ")).toContain("@e");
  });

  it("follows redirects back into the command that owns them", () => {
    // `execute as <targets>` redirects to `execute`, so its subcommands are
    // available again afterwards.
    expect(values("execute as @a ")).toContain("run");
    expect(values("execute as @a ")).toContain("at");
  });

  it("stays quiet for unknown commands and free-form arguments", () => {
    expect(values("notacommand ")).toEqual([]);
    expect(values("say ")).toEqual([]);
  });

  it("caps how many completions come back", () => {
    // Bare `give <player> ` would otherwise offer all 1537 items.
    expect(values("give Steve ").length).toBeLessThanOrEqual(40);
  });

  describe("on Bedrock", () => {
    it("offers Bedrock's own command set, not Java's", () => {
      expect(values("gam", PLAYERS, BEDROCK)).toEqual([
        "gamemode",
        "gamerule",
        "gametest",
        "gametips",
      ]);
      // Bedrock-only commands Java has never had.
      expect(values("daylock", PLAYERS, BEDROCK)).toEqual(["daylock"]);
      expect(values("tickingarea", PLAYERS, BEDROCK)).toEqual(["tickingarea"]);
      // Java-only commands a Bedrock server would reject.
      expect(values("bossbar", PLAYERS, BEDROCK)).toEqual([]);
      expect(values("advancement", PLAYERS, BEDROCK)).toEqual([]);
      expect(values("save-", PLAYERS, BEDROCK)).toEqual([]);
    });

    it("stops at command names, having no grammar to go on", () => {
      expect(values("give ", PLAYERS, BEDROCK)).toEqual([]);
    });
  });
});

describe("applyCompletion", () => {
  it("replaces the in-progress token and leaves room for the next", () => {
    expect(applyCompletion("giv", "give")).toBe("give ");
    expect(applyCompletion("give St", "Steve")).toBe("give Steve ");
  });

  it("appends when starting a fresh token", () => {
    expect(applyCompletion("give ", "Steve")).toBe("give Steve ");
  });

  it("keeps earlier tokens intact", () => {
    expect(applyCompletion("give Steve diamond", "diamond_sword")).toBe(
      "give Steve diamond_sword ",
    );
  });
});

describe("usageHint", () => {
  it("lists what the command accepts next", () => {
    expect(usageHint("give ", JAVA_26_2)).toBe("give: <targets>");
    expect(usageHint("data ", JAVA_26_2)).toBe("data: get | merge | modify | remove");
  });

  it("has nothing to say about unknown commands", () => {
    expect(usageHint("", JAVA_26_2)).toBeNull();
    expect(usageHint("notacommand x", JAVA_26_2)).toBeNull();
  });
});

describe("takesPlayerFirst", () => {
  it("recognises commands aimed at a player", () => {
    expect(takesPlayerFirst("give", JAVA_26_2)).toBe(true);
    expect(takesPlayerFirst("op", JAVA_26_2)).toBe(true);
    expect(takesPlayerFirst("seed", JAVA_26_2)).toBe(false);
  });
});
