import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig(async ({ mode }) => {
  const rootEnv = loadEnv(mode, resolve(__dirname, ".."), "");

  const plugins: any[] = [react()];

  const isTauri = !!process.env.TAURI_ENV_PLATFORM;

  return {
    resolve: {
      alias: {
        "@": resolve(__dirname, "src"),
        "src": resolve(__dirname, "src"),
      },
    },
    plugins,
    define: {
      __OPENROUTER_KEY__: JSON.stringify(rootEnv.OPENROUTER_API_KEY || ""),
      __JIUYAN_PHONE__: JSON.stringify(rootEnv.JIUYAN_PHONE || ""),
      __JIUYAN_PASSWORD__: JSON.stringify(rootEnv.JIUYAN_PASSWORD || ""),
      __API_BASE_URL__: JSON.stringify(
        rootEnv.VITE_API_BASE_URL ??
          (mode === "production" && isTauri ? "http://localhost:8080" : ""),
      ),
    },
    server: {
      proxy: {
        "/api": {
          target: "http://localhost:8080",
          changeOrigin: true,
        },
      },
    },
  };
});
