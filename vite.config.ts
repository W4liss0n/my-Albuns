import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
const devPort = 1437;
const hmrPort = 1438;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react()],

  build: {
    rollupOptions: {
      input: {
        dialog: "dialog.html",
        global: "global.html",
        project: "index.html",
        projectDialog: "project-dialog.html",
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: devPort,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: hmrPort,
        }
      : undefined,
    watch: {
      // Rust and local toolchain artifacts can be locked while Cargo links on Windows.
      ignored: [
        "**/src-tauri/**",
        "**/crates/**",
        "**/target/**",
        "**/.tools/**",
        "**/.scratch/**",
        "**/benchmark-data/**",
      ],
    },
  },
}));
