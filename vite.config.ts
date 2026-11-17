import preact from "@preact/preset-vite";
import { defineConfig } from "vite";

// Port 1420 matches devUrl in src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [preact()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
