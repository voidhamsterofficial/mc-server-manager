// MOTD (§-code) helpers: parsing for live preview, plus the escaping that
// server.properties needs. Java reads the file as latin-1, so § is stored
// as the escape §, and a second MOTD line is stored as a literal \n.

export const MOTD_COLORS: { code: string; name: string; hex: string }[] = [
  { code: "0", name: "Black", hex: "#000000" },
  { code: "1", name: "Dark Blue", hex: "#0000AA" },
  { code: "2", name: "Dark Green", hex: "#00AA00" },
  { code: "3", name: "Dark Aqua", hex: "#00AAAA" },
  { code: "4", name: "Dark Red", hex: "#AA0000" },
  { code: "5", name: "Dark Purple", hex: "#AA00AA" },
  { code: "6", name: "Gold", hex: "#FFAA00" },
  { code: "7", name: "Gray", hex: "#AAAAAA" },
  { code: "8", name: "Dark Gray", hex: "#555555" },
  { code: "9", name: "Blue", hex: "#5555FF" },
  { code: "a", name: "Green", hex: "#55FF55" },
  { code: "b", name: "Aqua", hex: "#55FFFF" },
  { code: "c", name: "Red", hex: "#FF5555" },
  { code: "d", name: "Light Purple", hex: "#FF55FF" },
  { code: "e", name: "Yellow", hex: "#FFFF55" },
  { code: "f", name: "White", hex: "#FFFFFF" },
];

export const MOTD_FORMATS: { code: string; label: string; title: string }[] = [
  { code: "l", label: "B", title: "Bold" },
  { code: "o", label: "I", title: "Italic" },
  { code: "n", label: "U", title: "Underline" },
  { code: "m", label: "S", title: "Strikethrough" },
  { code: "k", label: "K", title: "Obfuscated (magic)" },
  { code: "r", label: "⟲", title: "Reset formatting" },
];

export interface MotdSpan {
  text: string;
  color?: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  obfuscated: boolean;
}

function blankSpan(): MotdSpan {
  return {
    text: "",
    bold: false,
    italic: false,
    underline: false,
    strike: false,
    obfuscated: false,
  };
}

/** Parses one line of §-coded text into styled spans for the preview. */
export function parseMotdLine(text: string): MotdSpan[] {
  const spans: MotdSpan[] = [];
  let current = blankSpan();

  const flush = () => {
    if (current.text !== "") {
      spans.push(current);
    }
    current = { ...current, text: "" };
  };

  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char !== "§") {
      current.text += char;
      continue;
    }

    const code = text[i + 1]?.toLowerCase();
    i++;
    const colorEntry = MOTD_COLORS.find((entry) => entry.code === code);
    flush();
    if (colorEntry) {
      // A color code resets formatting, exactly like the game does.
      current = { ...blankSpan(), color: colorEntry.hex };
    } else if (code === "l") {
      current.bold = true;
    } else if (code === "o") {
      current.italic = true;
    } else if (code === "n") {
      current.underline = true;
    } else if (code === "m") {
      current.strike = true;
    } else if (code === "k") {
      current.obfuscated = true;
    } else if (code === "r") {
      current = { ...blankSpan(), color: current.color };
      current.color = undefined;
    }
  }

  flush();
  return spans;
}

/** Splits an editor-form MOTD into its (up to two) rendered lines. */
export function parseMotd(text: string): MotdSpan[][] {
  return text.split("\n").map(parseMotdLine);
}

/** server.properties form (§ escapes, literal \n) -> editor form. */
export function decodeMotdProperty(stored: string): string {
  return stored
    .replaceAll("\\u00A7", "§")
    .replaceAll("\\u00a7", "§")
    .replaceAll("\\n", "\n");
}

/** Editor form -> server.properties form. */
export function encodeMotdProperty(editorText: string): string {
  return editorText.replaceAll("§", "\\u00A7").replaceAll("\n", "\\n");
}
