import { describe, expect, it } from "vitest";
import { escapeHtml, highlight, lineCount, syntaxFor } from "./highlight";

/** The text a browser would render from the markup — highlighting must never
 *  change the content, only how it's painted. */
function textContentOf(html: string): string {
  return html
    .replace(/<[^>]*>/g, "")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, "&");
}

describe("syntaxFor", () => {
  it("maps extensions to a highlighting family", () => {
    expect(syntaxFor("server.properties")).toBe("ini");
    expect(syntaxFor("config.yml")).toBe("yaml");
    expect(syntaxFor("pack.mcmeta")).toBe("json");
    expect(syntaxFor("start.bat")).toBe("shell");
    expect(syntaxFor("latest.log")).toBe("log");
  });

  it("falls back to plain for anything unknown or extensionless", () => {
    expect(syntaxFor("eula.txt")).toBe("plain");
    expect(syntaxFor("LICENSE")).toBe("plain");
  });

  it("ignores extension case", () => {
    expect(syntaxFor("Config.YML")).toBe("yaml");
  });
});

describe("escapeHtml", () => {
  it("neutralises markup in file content", () => {
    expect(escapeHtml('<script>alert("x")</script>')).toBe(
      "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;",
    );
  });
});

describe("highlight", () => {
  it("preserves the text exactly, whatever the language", () => {
    const sources: [string, Parameters<typeof highlight>[1]][] = [
      ['{"a": 1, "b": [true, null]}', "json"],
      ["# comment\nkey: value\n  - item", "yaml"],
      ["motd=A <Minecraft> Server\n# note", "ini"],
      ['<tag attr="v">text & more</tag>', "markup"],
      ["if [ -f x ]; then echo 'hi'; fi", "shell"],
      ["## Heading\n- point with `code`", "markdown"],
      ["[12:00:00] [main/ERROR]: boom", "log"],
    ];
    for (const [source, kind] of sources) {
      expect(textContentOf(highlight(source, kind))).toBe(source);
    }
  });

  it("escapes content that looks like markup", () => {
    const highlighted = highlight("motd=<b>hi</b>", "ini");
    expect(highlighted).not.toContain("<b>");
    expect(highlighted).toContain("&lt;b&gt;");
  });

  it("marks up json keys, strings, numbers and literals distinctly", () => {
    const highlighted = highlight('{"name": "paper", "port": 25565, "on": true}', "json");
    expect(highlighted).toContain('<span class="tok-key">&quot;name&quot;</span>');
    expect(highlighted).toContain('<span class="tok-string">&quot;paper&quot;</span>');
    expect(highlighted).toContain('<span class="tok-number">25565</span>');
    expect(highlighted).toContain('<span class="tok-boolean">true</span>');
  });

  it("picks out keys and comments in properties files", () => {
    const highlighted = highlight("#Minecraft server properties\nmax-players=20", "ini");
    expect(highlighted).toContain('<span class="tok-comment">#Minecraft server properties</span>');
    expect(highlighted).toContain('<span class="tok-key">max-players</span>');
    expect(highlighted).toContain('<span class="tok-number">20</span>');
  });

  it("leaves identifiers containing digits whole", () => {
    // A UUID is one identifier, not a string of numbers glued to letters.
    // Matching digits mid-word used to shred `e90192fb-afcd-43b4-8925` into
    // coloured fragments.
    const highlighted = highlight("id: e90192fb-afcd-43b4-8925-61a0f0825794", "yaml");
    expect(highlighted).not.toContain("tok-number");
    expect(highlighted).toContain('<span class="tok-key">id</span>');

    // Version-like and hash-like values are identifiers too.
    expect(highlight("name: paper-1.21.1-build42", "yaml")).not.toContain("tok-number");
    expect(highlight("level-seed=8a3f9c2b", "ini")).not.toContain("tok-number");
  });

  it("still colours values that really are numbers", () => {
    expect(highlight("memoryMb: 2048", "yaml")).toContain('<span class="tok-number">2048</span>');
    expect(highlight("createdAtUnix: 1784645933", "yaml")).toContain("tok-number");
    expect(highlight("max-players=20", "ini")).toContain('<span class="tok-number">20</span>');
    expect(highlight("view-distance=-1", "ini")).toContain("tok-number");
  });

  it("colours log levels", () => {
    const highlighted = highlight("[main/ERROR]: nope", "log");
    expect(highlighted).toContain("tok-key");
  });

  it("leaves plain text alone but still escapes it", () => {
    expect(highlight("a < b & c", "plain")).toBe("a &lt; b &amp; c");
  });

  it("keeps blank lines, so line numbers stay aligned", () => {
    expect(highlight("a\n\nb", "plain").split("\n")).toHaveLength(3);
  });

  it("gives up on absurdly long lines rather than scanning them", () => {
    const long = `"${"x".repeat(3000)}"`;
    const highlighted = highlight(long, "json");
    expect(highlighted).not.toContain("<span");
    expect(textContentOf(highlighted)).toBe(long);
  });
});

describe("lineCount", () => {
  it("counts lines, including a trailing blank one", () => {
    expect(lineCount("")).toBe(1);
    expect(lineCount("a")).toBe(1);
    expect(lineCount("a\nb")).toBe(2);
    expect(lineCount("a\n")).toBe(2);
  });
});
