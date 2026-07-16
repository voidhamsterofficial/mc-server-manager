import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed dev-server port; failing fast beats silently picking another.
const TAURI_DEV_PORT = 1420;

export default defineConfig({
  // The frontend lives entirely in src/ (index.html included); src-tauri/ is
  // the Rust backend. Only tooling configs sit at the repo root.
  root: "src",
  plugins: [svelte({ configFile: "../svelte.config.js" })],
  clearScreen: false,
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  server: {
    port: TAURI_DEV_PORT,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
