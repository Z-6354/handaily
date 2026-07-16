import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";
import bundledPetPlugin from "./scripts/vite-bundled-pet";

// hanpet 应用根目录（vite.config.ts 位于 hanpet/）
const hanpetDir = __dirname;
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react(), bundledPetPlugin()],
  clearScreen: false,
  optimizeDeps: {
    // Pixi7 Live2D fork；避免仍解析已卸载的 pixi-live2d-display
    include: ["pixi-live2d-display-lipsyncpatch/cubism4", "pixi.js"],
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/hanimport/**", "**/data/**"],
    },
  },
  build: {
    target: "es2021",
    outDir: "dist",
    emptyOutDir: true,
    reportCompressedSize: false,
    minify: "esbuild",
    rollupOptions: {
        input: {
        main: resolve(hanpetDir, "index.html"),
        pet: resolve(hanpetDir, "pet.html"),
        "pet-menu": resolve(hanpetDir, "pet-menu.html"),
        "kanmusu-player": resolve(hanpetDir, "kanmusu-player.html"),
      },
      output: {
        manualChunks(id: string) {
          if (id.includes("node_modules/react-dom") || id.includes("node_modules/react/")) {
            return "vendor-react";
          }
          if (id.includes("node_modules/@tauri-apps/")) {
            return "vendor-tauri";
          }
        },
      },
    },
  },
}));
