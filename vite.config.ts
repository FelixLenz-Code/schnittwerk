import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed port and must not have the screen cleared.
export default defineConfig({
  // CSS is injected by the component JS (no separate virtual CSS modules):
  // avoids stale-cache CSS errors in the Tauri webview after restarts.
  plugins: [svelte({ emitCss: false })],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/target/**"] } },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "safari15", outDir: "dist", emptyOutDir: true },
});
