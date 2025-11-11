import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 3000,
    proxy: {
      "/api": {
        target: "http://localhost:34569",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
      "/ws": {
        target: "ws://localhost:34569",
        ws: true,
        changeOrigin: true,
      },
    },
  },
  build: {
    target: "esnext",
    minify: "terser",
    sourcemap: true,
  },
  optimizeDeps: {
    include: ["monaco-editor", "cytoscape", "chart.js"],
  },
});
