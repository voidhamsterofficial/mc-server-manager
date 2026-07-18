import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  // The frontend lives entirely in src/ (index.html included); src-tauri/ is
  // the Rust backend. Only tooling configs sit at the repo root. There is no
  // dev server: the app always loads the built files from dist/.
  root: "src",
  plugins: [svelte({ configFile: "../svelte.config.js" })],
  clearScreen: false,
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  test: {
    // jsdom so modules that touch browser globals (e.g. the Tauri API) import
    // cleanly; the current tests are pure logic but this keeps the door open.
    environment: "jsdom",
    include: ["**/*.test.ts"],
  },
});
