import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The frontend lives entirely in src/ (index.html included); src-tauri/ is
// the Rust backend. Only tooling configs sit at the repo root.
//
// `cargo tauri dev` points its `devUrl` at this dev server (see
// tauri.conf.json's `build.devUrl`), so the app window gets Vite's HMR —
// UI edits show up live instead of needing a full rebuild. Production
// builds (`npm run build` / `beforeBuildCommand`) still emit static files
// to dist/ with no dev server involved.
export default defineConfig({
  root: "src",
  plugins: [svelte({ configFile: "../svelte.config.js" })],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
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
