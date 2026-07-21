// Syntax highlighting for the file editor.
//
// Deliberately small and self-contained rather than a highlighting library:
// the editor only opens the config/text formats a server folder contains, and
// those need little more than comments, keys, strings and numbers picked out.
// A general-purpose highlighter would be far more code than the job needs.
//
// Everything here escapes its input — the source is arbitrary file content
// being injected as HTML, so nothing may reach the DOM unescaped.

/** Highlighting families. Several extensions share one (`.properties`,
 *  `.ini` and `.conf` are all key/value with comments). */
export type SyntaxKind = "json" | "yaml" | "ini" | "markup" | "shell" | "markdown" | "log" | "plain";

const KIND_BY_EXTENSION: Record<string, SyntaxKind> = {
  json: "json",
  json5: "json",
  mcmeta: "json",
  yml: "yaml",
  yaml: "yaml",
  properties: "ini",
  ini: "ini",
  conf: "ini",
  cfg: "ini",
  toml: "ini",
  xml: "markup",
  html: "markup",
  sh: "shell",
  bat: "shell",
  md: "markdown",
  log: "log",
};

/** The highlighting family for a file name, by extension. */
export function syntaxFor(fileName: string): SyntaxKind {
  const extension = fileName.includes(".") ? fileName.split(".").pop()! : "";
  return KIND_BY_EXTENSION[extension.toLowerCase()] ?? "plain";
}

interface Rule {
  /** Must be sticky (`y`) so matching is anchored at the scan position. */
  pattern: RegExp;
  /** Token class, or null to emit the match unstyled. */
  className: string | null;
}

// Note the trailing `[\w.-]+` rule most families end with. It consumes a
// whole identifier in one step so the scanner can never restart in the middle
// of one — without it, the digits inside `e90192fb-afcd-43b4` match the number
// rule and a UUID comes out shredded into coloured fragments. The number rule
// carries the matching guard: it only matches when the digits run to the end
// of the token, so `1.21.1-build42` stays one identifier rather than becoming
// a number with debris either side.

const RULES: Record<SyntaxKind, Rule[]> = {
  json: [
    { pattern: /"(?:[^"\\]|\\.)*"(?=\s*:)/y, className: "key" },
    { pattern: /"(?:[^"\\]|\\.)*"/y, className: "string" },
    { pattern: /\b(?:true|false|null)\b/y, className: "boolean" },
    { pattern: /-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?(?![\w.-])/y, className: "number" },
    { pattern: /[{}[\],:]/y, className: "punct" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  yaml: [
    { pattern: /#.*/y, className: "comment" },
    { pattern: /^\s*-\s/y, className: "punct" },
    { pattern: /^\s*[\w.$-]+(?=\s*:)/y, className: "key" },
    { pattern: /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/y, className: "string" },
    { pattern: /\b(?:true|false|null|yes|no|on|off)\b/y, className: "boolean" },
    { pattern: /-?\d+(?:\.\d+)?(?![\w.-])/y, className: "number" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  ini: [
    { pattern: /[#;!].*/y, className: "comment" },
    { pattern: /^\s*\[[^\]]*\]/y, className: "heading" },
    { pattern: /^\s*[\w.$-]+(?=\s*[=:])/y, className: "key" },
    { pattern: /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/y, className: "string" },
    { pattern: /\b(?:true|false)\b/y, className: "boolean" },
    { pattern: /-?\d+(?:\.\d+)?(?![\w.-])/y, className: "number" },
    { pattern: /[=:]/y, className: "punct" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  markup: [
    { pattern: /<!--[\s\S]*?-->/y, className: "comment" },
    { pattern: /<[/!?]?[\w:-]+/y, className: "tag" },
    { pattern: /[\w:-]+(?==)/y, className: "attr" },
    { pattern: /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/y, className: "string" },
    { pattern: /\/?>/y, className: "tag" },
    { pattern: /&[a-zA-Z#\d]+;/y, className: "boolean" },
  ],
  shell: [
    { pattern: /#.*|^\s*(?:REM|rem)\b.*/y, className: "comment" },
    { pattern: /"(?:[^"\\]|\\.)*"|'[^']*'/y, className: "string" },
    {
      pattern:
        /\b(?:if|then|else|elif|fi|for|in|do|done|while|case|esac|function|return|exit|echo|set|export|cd|goto|call|pause|start)\b/y,
      className: "keyword",
    },
    { pattern: /[$%][\w{}]+/y, className: "key" },
    { pattern: /-?\d+(?:\.\d+)?(?![\w.-])/y, className: "number" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  markdown: [
    { pattern: /^#{1,6}\s.*/y, className: "heading" },
    { pattern: /`[^`]*`/y, className: "string" },
    { pattern: /\*\*[^*]+\*\*|__[^_]+__/y, className: "keyword" },
    { pattern: /^\s*[-*+]\s/y, className: "punct" },
    { pattern: /\[[^\]]*\]\([^)]*\)/y, className: "key" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  log: [
    { pattern: /\[[^\]]*\]/y, className: "key" },
    { pattern: /\b(?:ERROR|SEVERE|FATAL)\b/y, className: "level-error" },
    { pattern: /\b(?:WARN|WARNING)\b/y, className: "level-warn" },
    { pattern: /\b(?:INFO|DEBUG|TRACE)\b/y, className: "level-info" },
    { pattern: /\b\d{1,2}:\d{2}:\d{2}\b/y, className: "number" },
    { pattern: /[\w.-]+/y, className: null },
  ],
  plain: [],
};

const HTML_ESCAPES: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

/** Escapes text for injection as HTML. Every path out of this module goes
 *  through here — the input is untrusted file content. */
export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (character) => HTML_ESCAPES[character]);
}

/** Longest source line we'll tokenize. A minified JSON blob on one line would
 *  otherwise make every keystroke re-scan the whole file; past this it's shown
 *  as plain escaped text, which is still perfectly readable. */
const MAX_HIGHLIGHTED_LINE_LENGTH = 2000;

function highlightLine(line: string, rules: Rule[]): string {
  if (rules.length === 0 || line.length > MAX_HIGHLIGHTED_LINE_LENGTH) {
    return escapeHtml(line);
  }

  let result = "";
  let position = 0;
  let plainRun = "";

  while (position < line.length) {
    const matched = matchRuleAt(line, position, rules);
    if (matched === null) {
      plainRun += line[position];
      position += 1;
      continue;
    }

    // Flush the unstyled characters collected since the last token.
    result += escapeHtml(plainRun);
    plainRun = "";

    const escaped = escapeHtml(matched.text);
    result += matched.className === null ? escaped : `<span class="tok-${matched.className}">${escaped}</span>`;
    position += matched.text.length;
  }

  result += escapeHtml(plainRun);
  return result;
}

interface Match {
  text: string;
  className: string | null;
}

function matchRuleAt(line: string, position: number, rules: Rule[]): Match | null {
  for (const rule of rules) {
    rule.pattern.lastIndex = position;
    const found = rule.pattern.exec(line);
    // A sticky pattern can only match at lastIndex, but a zero-length match
    // would loop forever — treat it as no match.
    if (found !== null && found[0].length > 0) {
      return { text: found[0], className: rule.className };
    }
  }
  return null;
}

/**
 * Highlights source as HTML, one line at a time. Returns markup whose text
 * content is byte-for-byte the input, so it can be layered underneath a
 * transparent textarea without the two drifting apart.
 */
export function highlight(source: string, kind: SyntaxKind): string {
  const rules = RULES[kind];
  return source
    .split("\n")
    .map((line) => highlightLine(line, rules))
    .join("\n");
}

/** Line numbers for a source string — always at least one. */
export function lineCount(source: string): number {
  return source.split("\n").length;
}
