import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Static SPA for Tauri (Phase 1) and, later, the Phase 2 server.
// See https://v2.tauri.app/start/frontend/vite/ for the Tauri-specific settings.
export default defineConfig({
  plugins: [svelte()],
  // Keep Rust errors visible in the terminal during `tauri dev`.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: 'localhost',
  },
  // Lets `tauri dev` pass its env vars through.
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Oldest engines we support: WebView2 (Chromium), Safari 15+ (macOS WebKit), WebKitGTK.
    target: ['es2022', 'chrome105', 'safari15'],
    outDir: 'dist',
    emptyOutDir: true,
    // Never inline assets as data: URLs into JS; everything ships as files under 'self'.
    assetsInlineLimit: 0,
    sourcemap: false,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
