import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Same build serves the Tauri desktop app and the plain web app.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
  envPrefix: ["VITE_", "TAURI_"],
});
