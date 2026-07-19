// Helpers for building Minecraft console commands safely from user- and
// player-supplied names. Console commands are newline-delimited on the
// server's stdin, so a name must never be able to smuggle in a second command.

// Control characters (incl. CR/LF, which would inject a second command).
const CONTROL_CHARS = new RegExp("[\\u0000-\\u001f\\u007f]+", "g");

/** Collapse anything that could break out of one command line into a space. */
function sanitizeArg(value: string): string {
  return value.replace(CONTROL_CHARS, " ").trim();
}

/**
 * Formats a player name as a single console-command argument. Names containing
 * spaces (Bedrock gamertags) are wrapped in quotes so the server reads them as
 * one token; vanilla names never contain spaces, so quoting is a harmless
 * no-op there. Control characters are always stripped first.
 */
export function commandArg(name: string): string {
  const clean = sanitizeArg(name);
  return clean.includes(" ") ? `"${clean}"` : clean;
}

/**
 * Sanitizes free-text that trails a command (e.g. a kick/ban reason): strips
 * control characters that could inject a second command, but keeps spaces so
 * multi-word reasons survive intact.
 */
export function commandText(value: string): string {
  return sanitizeArg(value);
}
