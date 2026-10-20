import { defineConfig, loadEnv, type PluginOption } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { resolve } from "path"

export default defineConfig(async ({ mode }) => {
  const rootEnv = loadEnv(mode, resolve(__dirname, ".."), "")

  const plugins: PluginOption[] = [react(), tailwindcss()]

  const isTauri = !!process.env.TAURI_ENV_PLATFORM

  return {
    resolve: {
      alias: {
        "@": resolve(__dirname, "src"),
        src: resolve(__dirname, "src"),
      },
    },
    plugins,
    define: {
      __OPENROUTER_KEY__: JSON.stringify(rootEnv.OPENROUTER_API_KEY || ""),
      __JIUYAN_PHONE__: JSON.stringify(rootEnv.JIUYAN_PHONE || ""),
      __JIUYAN_PASSWORD__: JSON.stringify(rootEnv.JIUYAN_PASSWORD || ""),
      __API_BASE_URL__: JSON.stringify(
        rootEnv.VITE_API_BASE_URL ??
          (mode === "production" && isTauri ? "http://localhost:8080" : "")
      ),
    },
    build: {
      sourcemap: false,
      // Mermaid is only imported when a message contains a ```mermaid block.
      // Its core (~636 kB) and the optional cynefin diagram (~691 kB) are
      // therefore lazy, on-demand chunks rather than initial payload. Keep the
      // warning threshold above them so real initial-bundle regressions still
      // surface instead of being drowned out by Mermaid internals.
      chunkSizeWarningLimit: 720,
    },
    server: {
      proxy: {
        "/api": {
          target: "http://localhost:8080",
          changeOrigin: true,
          ws: true,
        },
      },
    },
  }
})
