import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri expects a fixed dev port and wants its own env vars available.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "chrome105",
    sourcemap: false,
  },
});
