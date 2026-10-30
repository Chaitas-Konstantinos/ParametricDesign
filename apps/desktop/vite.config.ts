import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and no clearing of its console output. The Rust
// side (src-tauri, with its build output) is not watched: Windows locks those files.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, fs: { allow: ["../.."] }, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "es2022", outDir: "dist" },
});
