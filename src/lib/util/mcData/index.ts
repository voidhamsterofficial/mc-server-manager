// The command data available for autocomplete, one dataset per Minecraft
// version.
//
// A version's grammar is only correct for that version — commands come and go
// between releases — so there is no "closest match" fallback here. A server
// whose version has no dataset gets no suggestions at all, which is the honest
// answer: better to offer nothing than to offer commands the server will
// reject.
//
// Datasets are loaded on demand rather than bundled into the main chunk, so
// adding versions costs nothing until a server actually needs one.

import type { McCommandData } from "./types";

export type { ArgKind, CommandNode, McCommandData } from "./types";

/** The key Bedrock servers look up, whatever their version number. */
export const BEDROCK_DATA_KEY = "bedrock";

type DatasetLoader = () => Promise<{ COMMAND_DATA: McCommandData }>;

/** Version string (as stored on a server's config) -> its dataset. */
const DATASETS: Record<string, DatasetLoader> = {
  "26.2": () => import("./v26_2"),
  [BEDROCK_DATA_KEY]: () => import("./bedrock"),
};

/** Versions autocomplete can be offered for, for display in the UI. */
export const SUPPORTED_VERSIONS = Object.keys(DATASETS).filter(
  (key) => key !== BEDROCK_DATA_KEY,
);

export function hasCommandData(versionKey: string): boolean {
  return DATASETS[versionKey] !== undefined;
}

/** The dataset for a version, or null when there isn't one. */
export async function loadCommandData(versionKey: string): Promise<McCommandData | null> {
  const load = DATASETS[versionKey];
  if (load === undefined) {
    return null;
  }
  const dataset = await load();
  return dataset.COMMAND_DATA;
}
