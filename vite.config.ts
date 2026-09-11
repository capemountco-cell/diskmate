import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built frontend from `dist/` over a custom protocol, so the
// build must be relative-pathed and must not assume a web server.
export default defineConfig({
  plugins: [react()],
  // Fixed port: `tauri dev` waits for exactly this URL.
  server: { port: 5173, strictPort: true },
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "dist",
    // Safari 15 is the floor implied by the app's macOS 12 minimum.
    target: "safari15",
    sourcemap: true,
    emptyOutDir: true,
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
